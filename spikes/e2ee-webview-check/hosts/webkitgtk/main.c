#define _POSIX_C_SOURCE 200809L

#include <errno.h>
#include <fcntl.h>
#include <glib.h>
#include <gst/gst.h>
#include <gtk/gtk.h>
#include <jsc/jsc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>
#include <webkit/webkit.h>

enum {
    MAX_ASSET_BYTES = 64 * 1024,
    MAX_PROBE_BYTES = 8 * 1024,
    MAX_METADATA_BYTES = 128,
    PROBE_TIMEOUT_SECONDS = 30,
};

static const char *const PROBE_ORIGIN = "filament-probe://local";
static const char *const HOST_VERSION = "webkitgtk-diagnostic-v1";

typedef struct {
    const char *name;
    const char *content_type;
    char *contents;
    gsize length;
} ProbeAsset;

typedef struct {
    GMainLoop *loop;
    GtkWindow *window;
    WebKitWebView *web_view;
    ProbeAsset assets[4];
    guint timeout_source;
    gboolean finished;
    int output_descriptor;
    int exit_code;
} ProbeHost;

static void finish_host(ProbeHost *host, int exit_code, const char *message);

static char *bounded_text(const char *value, gsize limit)
{
    GString *bounded = g_string_sized_new(limit);
    const char *cursor = value != NULL ? value : "unknown";

    while (*cursor != '\0' && bounded->len < limit) {
        gunichar character = g_utf8_get_char_validated(cursor, -1);
        if (character == (gunichar)-1 || character == (gunichar)-2) {
            g_string_append_c(bounded, '?');
            cursor += 1;
            continue;
        }
        if (character == '\n' || character == '\r' || g_unichar_iscntrl(character)) {
            g_string_append_c(bounded, ' ');
        } else {
            char encoded[6] = {0};
            int encoded_length = g_unichar_to_utf8(character, encoded);
            if (bounded->len + (gsize)encoded_length > limit) {
                break;
            }
            g_string_append_len(bounded, encoded, encoded_length);
        }
        cursor = g_utf8_next_char(cursor);
    }
    return g_string_free(bounded, FALSE);
}

static char *json_string(const char *value, gsize limit)
{
    char *bounded = bounded_text(value, limit);
    GString *encoded = g_string_new("\"");

    for (const unsigned char *cursor = (const unsigned char *)bounded; *cursor != '\0'; cursor++) {
        switch (*cursor) {
        case '\"':
            g_string_append(encoded, "\\\"");
            break;
        case '\\':
            g_string_append(encoded, "\\\\");
            break;
        default:
            if (*cursor < 0x20) {
                g_string_append_printf(encoded, "\\u%04x", *cursor);
            } else {
                g_string_append_c(encoded, (char)*cursor);
            }
            break;
        }
    }
    g_string_append_c(encoded, '\"');
    g_free(bounded);
    return g_string_free(encoded, FALSE);
}

static gboolean load_asset(const char *directory, ProbeAsset *asset, GError **error)
{
    gboolean loaded = FALSE;
    char *path = g_build_filename(directory, asset->name, NULL);
    int descriptor = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    struct stat metadata;

    if (descriptor < 0) {
        g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(errno),
                    "probe asset cannot be opened: %s", asset->name);
        goto cleanup;
    }
    if (fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) ||
        metadata.st_size <= 0 || metadata.st_size > MAX_ASSET_BYTES) {
        g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                    "probe asset is invalid: %s", asset->name);
        goto cleanup;
    }

    asset->length = (gsize)metadata.st_size;
    asset->contents = g_malloc(asset->length);
    gsize offset = 0;
    while (offset < asset->length) {
        ssize_t amount = read(descriptor, asset->contents + offset, asset->length - offset);
        if (amount <= 0) {
            g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(errno),
                        "probe asset changed while loading: %s", asset->name);
            goto cleanup;
        }
        offset += (gsize)amount;
    }
    char extra;
    if (read(descriptor, &extra, 1) != 0) {
        g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_INVAL,
                    "probe asset changed while loading: %s", asset->name);
        goto cleanup;
    }
    loaded = TRUE;

cleanup:
    if (!loaded) {
        g_clear_pointer(&asset->contents, g_free);
        asset->length = 0;
    }
    if (descriptor >= 0) {
        close(descriptor);
    }
    g_free(path);
    return loaded;
}

static ProbeAsset *find_asset(ProbeHost *host, const char *path)
{
    const char *name = path != NULL && path[0] == '/' ? path + 1 : path;
    for (gsize index = 0; index < G_N_ELEMENTS(host->assets); index++) {
        if (g_strcmp0(host->assets[index].name, name) == 0) {
            return &host->assets[index];
        }
    }
    return NULL;
}

static void serve_probe_asset(WebKitURISchemeRequest *request, gpointer user_data)
{
    ProbeHost *host = user_data;
    ProbeAsset *asset = find_asset(host, webkit_uri_scheme_request_get_path(request));
    if (asset == NULL) {
        GError *error = g_error_new_literal(G_IO_ERROR, G_IO_ERROR_NOT_FOUND,
                                            "probe resource is not allowlisted");
        webkit_uri_scheme_request_finish_error(request, error);
        g_error_free(error);
        return;
    }

    GInputStream *stream = g_memory_input_stream_new_from_data(
        asset->contents, (gssize)asset->length, NULL);
    webkit_uri_scheme_request_finish(request, stream, (gint64)asset->length,
                                     asset->content_type);
    g_object_unref(stream);
}

static gboolean navigation_is_allowed(WebKitNavigationPolicyDecision *navigation)
{
    WebKitNavigationAction *action =
        webkit_navigation_policy_decision_get_navigation_action(navigation);
    WebKitURIRequest *request = webkit_navigation_action_get_request(action);
    const char *uri = webkit_uri_request_get_uri(request);
    return uri != NULL && g_str_has_prefix(uri, PROBE_ORIGIN) &&
           uri[strlen(PROBE_ORIGIN)] == '/';
}

static gboolean decide_policy(WebKitWebView *web_view, WebKitPolicyDecision *decision,
                              WebKitPolicyDecisionType type, gpointer user_data)
{
    (void)web_view;
    (void)user_data;
    if (type == WEBKIT_POLICY_DECISION_TYPE_NEW_WINDOW_ACTION ||
        (type == WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION &&
         !navigation_is_allowed(WEBKIT_NAVIGATION_POLICY_DECISION(decision)))) {
        webkit_policy_decision_ignore(decision);
        return TRUE;
    }
    return FALSE;
}

static gboolean deny_permission(WebKitWebView *web_view, WebKitPermissionRequest *request,
                                gpointer user_data)
{
    (void)web_view;
    (void)user_data;
    webkit_permission_request_deny(request);
    return TRUE;
}

static GtkWidget *deny_new_window(WebKitWebView *web_view,
                                  WebKitNavigationAction *navigation_action,
                                  gpointer user_data)
{
    (void)web_view;
    (void)navigation_action;
    (void)user_data;
    return NULL;
}

static gboolean disable_context_menu(WebKitWebView *web_view, WebKitContextMenu *context_menu,
                                     gpointer event, WebKitHitTestResult *hit_test_result,
                                     gpointer user_data)
{
    (void)web_view;
    (void)context_menu;
    (void)event;
    (void)hit_test_result;
    (void)user_data;
    return TRUE;
}

static void emit_record(ProbeHost *host, const char *probe)
{
    char *os_release = NULL;
    gsize os_release_length = 0;
    if (!g_file_get_contents("/etc/os-release", &os_release, &os_release_length, NULL) ||
        os_release_length > 64 * 1024) {
        g_clear_pointer(&os_release, g_free);
        os_release = g_strdup("Linux");
    }

    const char *pretty_start = strstr(os_release, "PRETTY_NAME=");
    char *os_value = NULL;
    if (pretty_start != NULL) {
        pretty_start += strlen("PRETTY_NAME=");
        const char *line_end = strchr(pretty_start, '\n');
        gsize line_length = line_end != NULL ? (gsize)(line_end - pretty_start)
                                               : strlen(pretty_start);
        os_value = g_strndup(pretty_start, line_length);
        g_strstrip(os_value);
        if (os_value[0] == '\"' && strlen(os_value) >= 2 &&
            os_value[strlen(os_value) - 1] == '\"') {
            os_value[strlen(os_value) - 1] = '\0';
            memmove(os_value, os_value + 1, strlen(os_value));
        }
    } else {
        os_value = g_strdup("Linux");
    }

    char *runtime = g_strdup_printf("%u.%u.%u", webkit_get_major_version(),
                                    webkit_get_minor_version(), webkit_get_micro_version());
    char *sdk = g_strdup_printf("%u.%u.%u", WEBKIT_MAJOR_VERSION, WEBKIT_MINOR_VERSION,
                                WEBKIT_MICRO_VERSION);
    char *gstreamer = bounded_text(gst_version_string(), MAX_METADATA_BYTES);
    char *os_json = json_string(os_value, MAX_METADATA_BYTES);
    char *runtime_json = json_string(runtime, MAX_METADATA_BYTES);
    char *sdk_json = json_string(sdk, MAX_METADATA_BYTES);
    char *gstreamer_json = json_string(gstreamer, MAX_METADATA_BYTES);

    GString *record = g_string_sized_new(2048);
    g_string_append_printf(
        record,
        "{\n"
        "  \"host_schema_version\": 1,\n"
        "  \"target\": \"linux\",\n"
        "  \"runtime\": \"webkitgtk\",\n"
        "  \"os_version\": %s,\n"
        "  \"runtime_version\": %s,\n"
        "  \"host_version\": \"%s\",\n"
        "  \"host_sdk_version\": %s,\n"
        "  \"gstreamer_version\": %s,\n"
        "  \"shipping_media_path\": \"native_livekit_gcm\",\n"
        "  \"probe\": %s\n"
        "}\n",
        os_json, runtime_json, HOST_VERSION, sdk_json, gstreamer_json, probe);

    if (record->len > MAX_PROBE_BYTES) {
        finish_host(host, 1, "probe evidence exceeded its bound");
    } else {
        gsize offset = 0;
        while (offset < record->len) {
            ssize_t amount = write(host->output_descriptor, record->str + offset,
                                   record->len - offset);
            if (amount < 0 && errno == EINTR) {
                continue;
            }
            if (amount <= 0) {
                finish_host(host, 1, "probe evidence could not be written");
                break;
            }
            offset += (gsize)amount;
        }
        if (offset == record->len) {
            finish_host(host, 0, NULL);
        }
    }

    g_string_free(record, TRUE);
    g_free(gstreamer_json);
    g_free(sdk_json);
    g_free(runtime_json);
    g_free(os_json);
    g_free(gstreamer);
    g_free(sdk);
    g_free(runtime);
    g_free(os_value);
    g_free(os_release);
}

static void probe_finished(GObject *object, GAsyncResult *result, gpointer user_data)
{
    ProbeHost *host = user_data;
    GError *error = NULL;
    JSCValue *value = webkit_web_view_call_async_javascript_function_finish(
        WEBKIT_WEB_VIEW(object), result, &error);
    if (value == NULL) {
        char *message = bounded_text(error != NULL ? error->message : "unknown error", 256);
        finish_host(host, 1, message);
        g_free(message);
        g_clear_error(&error);
        return;
    }
    if (!jsc_value_is_string(value)) {
        finish_host(host, 1, "probe returned a non-string result");
        g_object_unref(value);
        return;
    }

    char *probe = jsc_value_to_string(value);
    if (probe == NULL || probe[0] != '{' || strlen(probe) > MAX_PROBE_BYTES) {
        finish_host(host, 1, "probe returned no bounded JSON result");
    } else {
        emit_record(host, probe);
    }
    g_free(probe);
    g_object_unref(value);
}

static void run_probe(WebKitWebView *web_view, WebKitLoadEvent event, gpointer user_data)
{
    ProbeHost *host = user_data;
    if (event != WEBKIT_LOAD_FINISHED || host->finished) {
        return;
    }
    const char *body =
        "return JSON.stringify(await globalThis.runFilamentEncodedTransformProbe())";
    webkit_web_view_call_async_javascript_function(
        web_view, body, -1, NULL, NULL, "filament-probe://local/host-eval.js", NULL,
        probe_finished, host);
}

static gboolean probe_timed_out(gpointer user_data)
{
    ProbeHost *host = user_data;
    host->timeout_source = 0;
    finish_host(host, 1, "probe host timed out");
    return G_SOURCE_REMOVE;
}

static void finish_host(ProbeHost *host, int exit_code, const char *message)
{
    if (host->finished) {
        return;
    }
    host->finished = TRUE;
    host->exit_code = exit_code;
    if (message != NULL) {
        char *bounded = bounded_text(message, 256);
        fprintf(stderr, "%s\n", bounded);
        g_free(bounded);
    }
    if (host->timeout_source != 0) {
        g_source_remove(host->timeout_source);
        host->timeout_source = 0;
    }
    webkit_web_view_stop_loading(host->web_view);
    gtk_window_destroy(host->window);
    g_main_loop_quit(host->loop);
}

static gboolean load_failed(WebKitWebView *web_view, WebKitLoadEvent event,
                            const char *failing_uri, GError *error, gpointer user_data)
{
    (void)web_view;
    (void)event;
    (void)failing_uri;
    ProbeHost *host = user_data;
    finish_host(host, 1, error != NULL ? error->message : "probe navigation failed");
    return TRUE;
}

static gboolean initialize_host(ProbeHost *host, const char *asset_directory, GError **error)
{
    host->assets[0] = (ProbeAsset){"probe.html", "text/html", NULL, 0};
    host->assets[1] = (ProbeAsset){"probe.css", "text/css", NULL, 0};
    host->assets[2] = (ProbeAsset){"probe.js", "text/javascript", NULL, 0};
    host->assets[3] =
        (ProbeAsset){"rtp-transform-worker.js", "text/javascript", NULL, 0};
    for (gsize index = 0; index < G_N_ELEMENTS(host->assets); index++) {
        if (!load_asset(asset_directory, &host->assets[index], error)) {
            return FALSE;
        }
    }

    WebKitNetworkSession *session = webkit_network_session_new_ephemeral();
    WebKitSettings *settings = webkit_settings_new();
    webkit_settings_set_enable_developer_extras(settings, FALSE);
    webkit_settings_set_enable_html5_database(settings, FALSE);
    webkit_settings_set_enable_page_cache(settings, FALSE);
    webkit_settings_set_enable_webrtc(settings, TRUE);
    host->web_view = WEBKIT_WEB_VIEW(g_object_new(
        WEBKIT_TYPE_WEB_VIEW, "network-session", session, "settings", settings,
        "default-content-security-policy",
        "default-src 'none'; script-src 'self'; worker-src 'self'; style-src 'self'",
        NULL));
    g_object_unref(settings);
    g_object_unref(session);

    WebKitWebContext *context = webkit_web_view_get_context(host->web_view);
    webkit_web_context_register_uri_scheme(context, "filament-probe", serve_probe_asset,
                                           host, NULL);
    WebKitSecurityManager *security = webkit_web_context_get_security_manager(context);
    webkit_security_manager_register_uri_scheme_as_local(security, "filament-probe");
    webkit_security_manager_register_uri_scheme_as_secure(security, "filament-probe");

    g_signal_connect(host->web_view, "decide-policy", G_CALLBACK(decide_policy), host);
    g_signal_connect(host->web_view, "permission-request", G_CALLBACK(deny_permission), host);
    g_signal_connect(host->web_view, "create", G_CALLBACK(deny_new_window), host);
    g_signal_connect(host->web_view, "context-menu", G_CALLBACK(disable_context_menu), host);
    g_signal_connect(host->web_view, "load-changed", G_CALLBACK(run_probe), host);
    g_signal_connect(host->web_view, "load-failed", G_CALLBACK(load_failed), host);

    host->window = GTK_WINDOW(gtk_window_new());
    gtk_window_set_title(host->window, "Filament WebKitGTK encoded-transform probe");
    gtk_window_set_default_size(host->window, 640, 480);
    gtk_window_set_child(host->window, GTK_WIDGET(host->web_view));
    gtk_window_present(host->window);
    host->timeout_source = g_timeout_add_seconds(PROBE_TIMEOUT_SECONDS, probe_timed_out, host);
    return TRUE;
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "expected the local Filament probe asset directory\n");
        return 2;
    }
    const char *configured_output = getenv("FILAMENT_PROBE_OUTPUT_FD");
    int source_descriptor =
        configured_output != NULL && strcmp(configured_output, "3") == 0 ? 3 : STDOUT_FILENO;
    int output_descriptor = fcntl(source_descriptor, F_DUPFD_CLOEXEC, 4);
    int null_descriptor = open("/dev/null", O_WRONLY | O_CLOEXEC);
    if (output_descriptor < 0 || null_descriptor < 0 ||
        dup2(null_descriptor, STDOUT_FILENO) < 0) {
        fprintf(stderr, "WebKitGTK probe could not isolate its evidence stream\n");
        if (output_descriptor >= 0) {
            close(output_descriptor);
        }
        if (null_descriptor >= 0) {
            close(null_descriptor);
        }
        return 1;
    }
    close(null_descriptor);

    if (!gtk_init_check()) {
        fprintf(stderr, "WebKitGTK probe requires a working display\n");
        close(output_descriptor);
        return 1;
    }
    gst_init(NULL, NULL);

    ProbeHost host = {0};
    host.output_descriptor = output_descriptor;
    host.exit_code = 1;
    host.loop = g_main_loop_new(NULL, FALSE);
    GError *error = NULL;
    if (!initialize_host(&host, argv[1], &error)) {
        char *message = bounded_text(error != NULL ? error->message : "host setup failed", 256);
        fprintf(stderr, "%s\n", message);
        g_free(message);
        g_clear_error(&error);
    } else {
        webkit_web_view_load_uri(host.web_view, "filament-probe://local/probe.html");
        g_main_loop_run(host.loop);
    }

    for (gsize index = 0; index < G_N_ELEMENTS(host.assets); index++) {
        g_free(host.assets[index].contents);
    }
    g_main_loop_unref(host.loop);
    close(host.output_descriptor);
    return host.exit_code;
}
