using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

internal static class Program
{
    private const int HostSchemaVersion = 1;
    private const string HostVersion = "webview2-diagnostic-v1";
    private const string HostSdkVersion = "1.0.4078.44";
    private const string ProbeOrigin = "https://filament-probe.local";
    private const int TimeoutMilliseconds = 30_000;

    [STAThread]
    private static int Main(string[] args)
    {
        if (args.Length != 1 || !TryResolveProbeAssets(args[0], out var probeAssets))
        {
            Console.Error.WriteLine("expected the local Filament probe asset directory");
            return 2;
        }

        ApplicationConfiguration.Initialize();
        using var form = new Form
        {
            ShowInTaskbar = false,
            WindowState = FormWindowState.Minimized,
            Text = "Filament WebView2 encoded-transform probe",
        };
        using var webView = new WebView2 { Dock = DockStyle.Fill };
        using var timer = new System.Windows.Forms.Timer { Interval = TimeoutMilliseconds };
        form.Controls.Add(webView);

        var exitCode = 1;
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            Console.Error.WriteLine("probe host timed out");
            form.Close();
        };

        form.Shown += async (_, _) =>
        {
            try
            {
                var userDataFolder = Path.Combine(
                    Path.GetTempPath(),
                    $"filament-webview2-probe-{Environment.ProcessId}");
                var environment = await CoreWebView2Environment.CreateAsync(
                    browserExecutableFolder: null,
                    userDataFolder: userDataFolder);
                await webView.EnsureCoreWebView2Async(environment);
                Harden(webView.CoreWebView2, probeAssets);
                webView.CoreWebView2.NavigationCompleted += async (_, navigation) =>
                {
                    if (!navigation.IsSuccess)
                    {
                        Console.Error.WriteLine($"navigation failed: {navigation.WebErrorStatus}");
                        form.Close();
                        return;
                    }

                    try
                    {
                        var probe = await RunProbe(webView.CoreWebView2);
                        var record = new
                        {
                            host_schema_version = HostSchemaVersion,
                            target = "windows",
                            runtime = "webview2",
                            os_version = Environment.OSVersion.VersionString,
                            runtime_version = webView.CoreWebView2.Environment.BrowserVersionString,
                            host_version = HostVersion,
                            host_sdk_version = HostSdkVersion,
                            shipping_media_path = "native_livekit_gcm",
                            probe = probe.RootElement,
                        };
                        Console.WriteLine(JsonSerializer.Serialize(record, new JsonSerializerOptions
                        {
                            WriteIndented = true,
                        }));
                        exitCode = 0;
                    }
                    catch (Exception error)
                    {
                        Console.Error.WriteLine(BoundedError(error));
                    }
                    finally
                    {
                        form.Close();
                    }
                };
                webView.CoreWebView2.Navigate($"{ProbeOrigin}/probe.html");
            }
            catch (Exception error)
            {
                Console.Error.WriteLine(BoundedError(error));
                form.Close();
            }
        };

        timer.Start();
        Application.Run(form);
        return exitCode;
    }

    private static void Harden(CoreWebView2 core, string probeAssets)
    {
        var settings = core.Settings;
        settings.AreDevToolsEnabled = false;
        settings.AreDefaultContextMenusEnabled = false;
        settings.IsStatusBarEnabled = false;
        settings.IsPasswordAutosaveEnabled = false;
        settings.IsGeneralAutofillEnabled = false;
        settings.IsWebMessageEnabled = false;
        core.SetVirtualHostNameToFolderMapping(
            "filament-probe.local",
            probeAssets,
            CoreWebView2HostResourceAccessKind.DenyCors);
        core.NavigationStarting += (_, navigation) =>
        {
            if (!navigation.Uri.StartsWith($"{ProbeOrigin}/", StringComparison.Ordinal))
            {
                navigation.Cancel = true;
            }
        };
        core.NewWindowRequested += (_, request) => request.Handled = true;
        core.PermissionRequested += (_, request) => request.State = CoreWebView2PermissionState.Deny;
        core.DownloadStarting += (_, download) => download.Cancel = true;
        core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += (_, request) =>
        {
            if (!Uri.TryCreate(request.Request.Uri, UriKind.Absolute, out var uri)
                || uri.Scheme != Uri.UriSchemeHttps
                || uri.Host != "filament-probe.local")
            {
                request.Response = core.Environment.CreateWebResourceResponse(
                    Stream.Null,
                    403,
                    "Forbidden",
                    "Content-Type: text/plain");
            }
        };
    }

    private static async Task<JsonDocument> RunProbe(CoreWebView2 core)
    {
        await core.ExecuteScriptAsync("document.querySelector('#run').click()");
        for (var attempt = 0; attempt < 60; attempt += 1)
        {
            await Task.Delay(250);
            var encoded = await core.ExecuteScriptAsync(
                "document.querySelector('#result').textContent");
            var text = JsonSerializer.Deserialize<string>(encoded);
            if (text?.StartsWith('{') == true && text.Length <= 4096)
            {
                return JsonDocument.Parse(text);
            }
        }

        throw new InvalidOperationException("probe returned no bounded JSON result");
    }

    private static bool TryResolveProbeAssets(string candidate, out string resolved)
    {
        resolved = string.Empty;
        try
        {
            var directory = new DirectoryInfo(candidate);
            if (!directory.Exists || directory.Attributes.HasFlag(FileAttributes.ReparsePoint))
            {
                return false;
            }

            foreach (var name in new[]
                     {
                         "probe.html",
                         "probe.css",
                         "probe.js",
                         "rtp-transform-worker.js",
                     })
            {
                var file = new FileInfo(Path.Combine(directory.FullName, name));
                if (!file.Exists
                    || file.Attributes.HasFlag(FileAttributes.ReparsePoint)
                    || file.Length is <= 0 or > 64 * 1024)
                {
                    return false;
                }
            }

            resolved = directory.FullName;
            return true;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    private static string BoundedError(Exception error)
    {
        var message = error.Message.ReplaceLineEndings(" ");
        return message[..Math.Min(message.Length, 256)];
    }
}
