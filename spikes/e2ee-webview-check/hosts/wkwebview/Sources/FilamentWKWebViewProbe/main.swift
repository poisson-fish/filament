import AppKit
import Foundation
import Network
import WebKit

private let hostSchemaVersion = 1
private let hostVersion = "wkwebview-diagnostic-v1"
private let hostSDKVersion = "system-webkit"
private let maximumAssetBytes = 64 * 1024
private let maximumRequestBytes = 4 * 1024
private let maximumResultBytes = 8 * 1024
private let probeTimeoutSeconds = 30.0

private enum HostFailure: Error, CustomStringConvertible {
    case invalidArguments
    case invalidAsset(String)
    case listener(String)
    case navigation(String)
    case probe(String)

    var description: String {
        switch self {
        case .invalidArguments:
            return "expected the local Filament probe asset directory"
        case let .invalidAsset(reason), let .listener(reason), let .navigation(reason), let .probe(reason):
            return reason
        }
    }
}

private struct ProbeAsset {
    let contentType: String
    let data: Data
}

private final class LocalProbeServer {
    private let listener: NWListener
    private let queue = DispatchQueue(label: "com.filament.wkwebview-probe.listener")
    private let assets: [String: ProbeAsset]
    private var readyHandler: ((Result<UInt16, Error>) -> Void)?

    init(assetDirectory: URL) throws {
        assets = try Self.loadAssets(from: assetDirectory)

        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = false
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: parameters)
        listener.newConnectionHandler = { [weak self] connection in
            self?.accept(connection)
        }
        listener.stateUpdateHandler = { [weak self] state in
            self?.stateChanged(state)
        }
    }

    func start(ready: @escaping (Result<UInt16, Error>) -> Void) {
        readyHandler = ready
        listener.start(queue: queue)
    }

    func stop() {
        listener.cancel()
    }

    private func stateChanged(_ state: NWListener.State) {
        switch state {
        case .ready:
            guard let port = listener.port?.rawValue else {
                finishReady(.failure(HostFailure.listener("listener has no bounded port")))
                return
            }
            finishReady(.success(port))
        case let .failed(error):
            finishReady(.failure(HostFailure.listener("listener failed: \(bounded(error.localizedDescription))")))
        default:
            break
        }
    }

    private func finishReady(_ result: Result<UInt16, Error>) {
        guard let handler = readyHandler else { return }
        readyHandler = nil
        DispatchQueue.main.async { handler(result) }
    }

    private func accept(_ connection: NWConnection) {
        guard Self.isLoopback(connection.endpoint) else {
            connection.cancel()
            return
        }
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: maximumRequestBytes) {
            [weak self] data, _, isComplete, error in
            guard let self else {
                connection.cancel()
                return
            }
            guard error == nil, let data, data.count <= maximumRequestBytes else {
                self.respond(status: "400 Bad Request", asset: nil, on: connection)
                return
            }
            guard isComplete || data.range(of: Data("\r\n\r\n".utf8)) != nil else {
                self.respond(status: "431 Request Header Fields Too Large", asset: nil, on: connection)
                return
            }
            guard let line = String(data: data, encoding: .utf8)?.components(separatedBy: "\r\n").first,
                  line.count <= 256,
                  line.hasPrefix("GET "),
                  line.hasSuffix(" HTTP/1.1")
            else {
                self.respond(status: "400 Bad Request", asset: nil, on: connection)
                return
            }
            let path = String(line.dropFirst(4).dropLast(9))
            guard let asset = self.assets[path] else {
                self.respond(status: "404 Not Found", asset: nil, on: connection)
                return
            }
            self.respond(status: "200 OK", asset: asset, on: connection)
        }
    }

    private func respond(status: String, asset: ProbeAsset?, on connection: NWConnection) {
        let body = asset?.data ?? Data()
        let contentType = asset?.contentType ?? "text/plain; charset=utf-8"
        let header = "HTTP/1.1 \(status)\r\n" +
            "Content-Type: \(contentType)\r\n" +
            "Content-Length: \(body.count)\r\n" +
            "Cache-Control: no-store\r\n" +
            "Content-Security-Policy: default-src 'none'; script-src 'self'; worker-src 'self'; style-src 'self'; connect-src 'none'; media-src 'none'\r\n" +
            "X-Content-Type-Options: nosniff\r\n" +
            "Connection: close\r\n\r\n"
        var response = Data(header.utf8)
        response.append(body)
        connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
    }

    private static func loadAssets(from directory: URL) throws -> [String: ProbeAsset] {
        let values = try directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        guard values.isDirectory == true, values.isSymbolicLink != true else {
            throw HostFailure.invalidAsset("probe asset directory is invalid")
        }

        let definitions = [
            ("/probe.html", "text/html; charset=utf-8"),
            ("/probe.css", "text/css; charset=utf-8"),
            ("/probe.js", "text/javascript; charset=utf-8"),
            ("/rtp-transform-worker.js", "text/javascript; charset=utf-8"),
        ]
        return try Dictionary(uniqueKeysWithValues: definitions.map { path, contentType in
            let file = directory.appendingPathComponent(String(path.dropFirst()), isDirectory: false)
            let fileValues = try file.resourceValues(forKeys: [
                .isRegularFileKey,
                .isSymbolicLinkKey,
                .fileSizeKey,
            ])
            guard fileValues.isRegularFile == true,
                  fileValues.isSymbolicLink != true,
                  let size = fileValues.fileSize,
                  size > 0,
                  size <= maximumAssetBytes
            else {
                throw HostFailure.invalidAsset("probe asset is invalid: \(path)")
            }
            let data = try Data(contentsOf: file, options: [.mappedIfSafe])
            guard data.count == size else {
                throw HostFailure.invalidAsset("probe asset changed while loading: \(path)")
            }
            return (path, ProbeAsset(contentType: contentType, data: data))
        })
    }

    private static func isLoopback(_ endpoint: NWEndpoint) -> Bool {
        guard case let .hostPort(host, _) = endpoint else { return false }
        return host == "127.0.0.1" || host == "::1"
    }
}

private final class ProbeController: NSObject, NSApplicationDelegate, WKNavigationDelegate, WKUIDelegate {
    private let server: LocalProbeServer
    private var webView: WKWebView?
    private var window: NSWindow?
    private var timeout: DispatchWorkItem?
    private var allowedOrigin: String?
    private var finished = false

    init(assetDirectory: URL) throws {
        server = try LocalProbeServer(assetDirectory: assetDirectory)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        server.start { [weak self] result in
            switch result {
            case let .success(port):
                self?.launchWebView(port: port)
            case let .failure(error):
                self?.finish(.failure(error))
            }
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    private func launchWebView(port: UInt16) {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = true
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.mediaTypesRequiringUserActionForPlayback = []

        let view = WKWebView(frame: NSRect(x: 0, y: 0, width: 640, height: 480), configuration: configuration)
        view.navigationDelegate = self
        view.uiDelegate = self
        if #available(macOS 13.3, *) {
            view.isInspectable = false
        }
        webView = view

        let diagnosticWindow = NSWindow(
            contentRect: view.frame,
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        diagnosticWindow.title = "Filament WKWebView encoded-transform probe"
        diagnosticWindow.contentView = view
        diagnosticWindow.center()
        diagnosticWindow.orderFrontRegardless()
        window = diagnosticWindow

        allowedOrigin = "http://127.0.0.1:\(port)"
        guard let url = URL(string: "\(allowedOrigin!)/probe.html") else {
            finish(.failure(HostFailure.navigation("invalid local probe URL")))
            return
        }
        let deadline = DispatchWorkItem { [weak self] in
            self?.finish(.failure(HostFailure.probe("probe host timed out")))
        }
        timeout = deadline
        DispatchQueue.main.asyncAfter(deadline: .now() + probeTimeoutSeconds, execute: deadline)
        view.load(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 10))
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        Task { @MainActor [weak self] in
            do {
                let value = try await webView.callAsyncJavaScript(
                    "return JSON.stringify(await globalThis.runFilamentEncodedTransformProbe())",
                    arguments: [:],
                    in: nil,
                    contentWorld: .page
                )
                guard let raw = value as? String,
                      let data = raw.data(using: .utf8),
                      data.count <= maximumResultBytes,
                      let probe = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
                else {
                    self?.finish(.failure(HostFailure.probe("probe returned no bounded JSON result")))
                    return
                }
                self?.emit(probe: probe)
            } catch {
                self?.finish(.failure(HostFailure.probe("probe evaluation failed: \(bounded(error.localizedDescription))")))
            }
        }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        finish(.failure(HostFailure.navigation("navigation failed: \(bounded(error.localizedDescription))")))
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        finish(.failure(HostFailure.navigation("navigation failed: \(bounded(error.localizedDescription))")))
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = navigationAction.request.url,
              let allowedOrigin,
              url.absoluteString.hasPrefix("\(allowedOrigin)/")
        else {
            decisionHandler(.cancel)
            return
        }
        decisionHandler(.allow)
    }

    func webView(
        _ webView: WKWebView,
        requestMediaCapturePermissionFor origin: WKSecurityOrigin,
        initiatedByFrame frame: WKFrameInfo,
        type: WKMediaCaptureType,
        decisionHandler: @escaping (WKPermissionDecision) -> Void
    ) {
        decisionHandler(.deny)
    }

    private func emit(probe: [String: Any]) {
        let webKitBundle = Bundle(identifier: "com.apple.WebKit") ?? Bundle(path: "/System/Library/Frameworks/WebKit.framework")
        let runtimeVersion = (webKitBundle?.object(forInfoDictionaryKey: "CFBundleVersion") as? String) ?? "unknown"
        let record: [String: Any] = [
            "host_schema_version": hostSchemaVersion,
            "target": "macos",
            "runtime": "wkwebview",
            "os_version": ProcessInfo.processInfo.operatingSystemVersionString,
            "runtime_version": runtimeVersion,
            "host_version": hostVersion,
            "host_sdk_version": hostSDKVersion,
            "shipping_media_path": "native_livekit_gcm",
            "probe": probe,
        ]
        guard JSONSerialization.isValidJSONObject(record),
              let data = try? JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys]),
              data.count <= maximumResultBytes,
              let output = String(data: data, encoding: .utf8)
        else {
            finish(.failure(HostFailure.probe("probe evidence exceeded its bound")))
            return
        }
        finish(.success(output))
    }

    private func finish(_ result: Result<String, Error>) {
        guard !finished else { return }
        finished = true
        timeout?.cancel()
        server.stop()
        webView?.stopLoading()
        window?.orderOut(nil)
        switch result {
        case let .success(output):
            FileHandle.standardOutput.write(Data((output + "\n").utf8))
            NSApplication.shared.terminate(0)
        case let .failure(error):
            FileHandle.standardError.write(Data((bounded(String(describing: error)) + "\n").utf8))
            NSApplication.shared.terminate(1)
        }
    }
}

private func bounded(_ value: String) -> String {
    let flattened = value.replacingOccurrences(of: "\n", with: " ").replacingOccurrences(of: "\r", with: " ")
    return String(flattened.prefix(256))
}

guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write(Data((HostFailure.invalidArguments.description + "\n").utf8))
    exit(2)
}

do {
    let assets = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true).standardizedFileURL
    let controller = try ProbeController(assetDirectory: assets)
    let application = NSApplication.shared
    application.setActivationPolicy(.accessory)
    application.delegate = controller
    application.run()
} catch {
    FileHandle.standardError.write(Data((bounded(String(describing: error)) + "\n").utf8))
    exit(1)
}
