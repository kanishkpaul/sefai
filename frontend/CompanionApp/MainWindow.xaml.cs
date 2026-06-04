using System.Collections.ObjectModel;
using System.Globalization;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Threading;
using CompanionApp.Models;
using CompanionApp.Services;
using CompanionApp.Views;
using Forms = System.Windows.Forms;

namespace CompanionApp;

public partial class MainWindow : Window
{
    private readonly SettingsStore _settingsStore;
    private readonly BackendProcessService _backendProcessService;
    private readonly StartupRegistrationService _startupRegistrationService;
    private readonly ObservableCollection<ChatMessage> _chatMessages = new();
    private readonly ObservableCollection<string> _timelineEvents = new();
    private readonly DispatcherTimer _pollTimer;
    private readonly DispatcherTimer _backendWatchdogTimer;
    private readonly Forms.NotifyIcon _notifyIcon;
    private AppSettings _settings;
    private bool _autonomyPaused;
    private bool _allowClose;
    private bool _isRecoveringBackend;
    private int _seenMessageCount;

    public MainWindow(
        SettingsStore settingsStore,
        AppSettings settings,
        BackendProcessService backendProcessService,
        StartupRegistrationService startupRegistrationService)
    {
        InitializeComponent();
        _settingsStore = settingsStore;
        _settings = settings;
        _backendProcessService = backendProcessService;
        _startupRegistrationService = startupRegistrationService;
        _backendProcessService.BackendExited += BackendProcessService_BackendExited;

        ChatListBox.ItemsSource = _chatMessages;
        TimelineListBox.ItemsSource = _timelineEvents;
        GoalsItemsControl.ItemsSource = Array.Empty<string>();

        _notifyIcon = new Forms.NotifyIcon
        {
            Text = "Sefai Companion",
            Icon = System.Drawing.SystemIcons.Information,
            Visible = true,
        };
        _notifyIcon.DoubleClick += (_, _) => RestoreFromTray();

        var menu = new Forms.ContextMenuStrip();
        menu.Items.Add("Open", null, (_, _) => RestoreFromTray());
        menu.Items.Add("Toggle quiet mode", null, async (_, _) => await ToggleQuietModeAsync());
        menu.Items.Add("Exit", null, async (_, _) => await ExitApplicationAsync());
        _notifyIcon.ContextMenuStrip = menu;

        _pollTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromSeconds(15),
        };
        _pollTimer.Tick += async (_, _) => await TryRefreshFromBackendAsync(showNotifications: true);

        _backendWatchdogTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromSeconds(5),
        };
        _backendWatchdogTimer.Tick += async (_, _) => await EnsureBackendProcessAsync();

        Loaded += async (_, _) =>
        {
            try
            {
                await InitializeFromBackendAsync();
                _pollTimer.Start();
                _backendWatchdogTimer.Start();
            }
            catch (Exception ex)
            {
                ShowBackendFailure(ex, "Initialization failed");
            }
        };
        Closing += MainWindow_Closing;
        StateChanged += MainWindow_StateChanged;
    }

    private async Task InitializeFromBackendAsync()
    {
        RuntimeTextBlock.Text = "Backend: connected";
        ModelPathTextBlock.Text = $"Model: {_settings.ModelPath}";
        QuietModeTextBlock.Text = _settings.QuietMode ? "Quiet mode: on" : "Quiet mode: off";

        var client = _backendProcessService.CreateClient();
        await client.SendAsync("initialize", new Dictionary<string, object?>());
        await RefreshFromBackendAsync(showNotifications: false);
    }

    private async Task RefreshFromBackendAsync(bool showNotifications)
    {
        var client = _backendProcessService.CreateClient();

        var stateResponse = await client.SendAsync("get_state", new Dictionary<string, object?>());
        ApplyState(stateResponse.Payload);

        var historyResponse = await client.SendAsync("get_history", new Dictionary<string, object?> { ["limit"] = 50 });
        ApplyHistory(historyResponse.Payload, showNotifications);
    }

    private async Task TryRefreshFromBackendAsync(bool showNotifications)
    {
        try
        {
            await RefreshFromBackendAsync(showNotifications);
        }
        catch (Exception ex)
        {
            if (await TryRecoverBackendAsync(ex, "Backend refresh failed"))
            {
                await RefreshFromBackendAsync(showNotifications);
                return;
            }
            ShowBackendFailure(ex, "Backend refresh failed");
        }
    }

    private void ApplyState(Dictionary<string, object?> payload)
    {
        var json = JsonSerializer.Serialize(payload);
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;

        CompanionNameTextBlock.Text = root.TryGetProperty("name", out var name) ? name.GetString() ?? "Companion" : "Companion";
        var mood = root.TryGetProperty("mood", out var moodElement) ? moodElement.GetString() ?? "engaged" : "engaged";
        MoodTextBlock.Text = CultureInfo.CurrentCulture.TextInfo.ToTitleCase(mood);
        RelationshipSummaryTextBlock.Text = root.TryGetProperty("relationship_summary", out var summary)
            ? summary.GetString() ?? ""
            : "";
        StatusTextBlock.Text = root.TryGetProperty("autonomy_enabled", out var autonomy) && autonomy.GetBoolean()
            ? "Resident and listening"
            : "Autonomy paused";
        _autonomyPaused = !(root.TryGetProperty("autonomy_enabled", out var autonomyEnabledElement) && autonomyEnabledElement.GetBoolean());
        AutonomyButton.Content = _autonomyPaused ? "Resume Autonomy" : "Pause Autonomy";

        if (root.TryGetProperty("active_goals", out var goals))
        {
            GoalsItemsControl.ItemsSource = goals.EnumerateArray().Select(item => $"* {item.GetString()}").ToArray();
        }

        var runtimeMode = root.TryGetProperty("runtime_mode", out var runtimeModeElement)
            ? runtimeModeElement.GetString() ?? "unknown"
            : "unknown";
        var runtimeError = root.TryGetProperty("runtime_error", out var runtimeErrorElement)
            ? runtimeErrorElement.GetString()
            : null;
        var ggufActive = root.TryGetProperty("gguf_active", out var ggufActiveElement) && ggufActiveElement.GetBoolean();
        var runtimeLabel = root.TryGetProperty("runtime_label", out var runtimeLabelElement)
            ? runtimeLabelElement.GetString() ?? runtimeMode
            : runtimeMode;
        var lastProbeOk = root.TryGetProperty("last_probe_ok", out var lastProbeOkElement) && lastProbeOkElement.GetBoolean();
        var lastProbeAt = root.TryGetProperty("last_probe_at", out var lastProbeAtElement)
            ? lastProbeAtElement.GetString()
            : null;
        var lastProbeDetail = root.TryGetProperty("last_probe_detail", out var lastProbeDetailElement)
            ? lastProbeDetailElement.GetString() ?? "Live probe not run yet."
            : "Live probe not run yet.";
        RuntimeTextBlock.Text = ggufActive
            ? "Backend: live GGUF runtime detected"
            : $"Backend: non-GGUF fallback path ({runtimeMode})";
        GgufStatusTextBlock.Text = ggufActive ? "GGUF STATUS: ACTIVE" : "GGUF STATUS: NOT RUNNING";
        RuntimeDetailTextBlock.Text = runtimeLabel;
        RuntimeProbeTextBlock.Text = lastProbeAt is not null
            ? $"Live check: {(lastProbeOk ? "PASS" : "FAIL")} at {lastProbeAt} | {lastProbeDetail}"
            : $"Live check: pending | {lastProbeDetail}";
        if (!string.IsNullOrWhiteSpace(runtimeError))
        {
            RuntimeTextBlock.Text += $" - {runtimeError}";
            RuntimeDetailTextBlock.Text += $" | {runtimeError}";
        }
    }

    private void ApplyHistory(Dictionary<string, object?> payload, bool showNotifications)
    {
        var json = JsonSerializer.Serialize(payload);
        using var document = JsonDocument.Parse(json);
        var messages = document.RootElement.GetProperty("messages");

        _chatMessages.Clear();
        var newTimeline = new List<string>();
        var totalCount = 0;

        foreach (var message in messages.EnumerateArray())
        {
            totalCount++;
            var role = message.GetProperty("role").GetString() ?? "companion";
            var content = message.GetProperty("content").GetString() ?? "";
            var initiatedBy = message.TryGetProperty("initiated_by", out var initiatedProp) ? initiatedProp.GetString() : null;
            var timestamp = message.TryGetProperty("timestamp", out var tsProp) && DateTime.TryParse(tsProp.GetString(), out var parsedTime)
                ? parsedTime.ToLocalTime()
                : DateTime.Now;

            var chatMessage = new ChatMessage
            {
                Speaker = role == "user" ? "You" : CompanionNameTextBlock.Text,
                Content = content,
                DeliveryTag = initiatedBy == "autonomous" ? "Autonomous" : role == "user" ? "You" : "Reply",
                Timestamp = timestamp,
                AccentBrush = role == "user"
                    ? System.Windows.Media.Brushes.SteelBlue
                    : (initiatedBy == "autonomous"
                        ? System.Windows.Media.Brushes.IndianRed
                        : System.Windows.Media.Brushes.SaddleBrown),
            };
            _chatMessages.Add(chatMessage);

            if (initiatedBy == "autonomous")
            {
                newTimeline.Add($"{timestamp:g}  {content}");
            }
        }

        TimelineListBox.ItemsSource = newTimeline;

        if (showNotifications && totalCount > _seenMessageCount && _chatMessages.LastOrDefault() is { DeliveryTag: "Autonomous" } latest)
        {
            if (_settings.NotificationsEnabled)
            {
                _notifyIcon.ShowBalloonTip(4000, CompanionNameTextBlock.Text, latest.Content, Forms.ToolTipIcon.Info);
            }
        }

        _seenMessageCount = totalCount;
    }

    private async void Send_Click(object sender, RoutedEventArgs e)
    {
        var message = ChatInputTextBox.Text.Trim();
        if (string.IsNullOrWhiteSpace(message))
        {
            return;
        }

        SendButton.IsEnabled = false;
        ComposerHintTextBlock.Text = "Thinking...";
        try
        {
            var client = _backendProcessService.CreateClient();
            await client.SendAsync("send_user_message", new Dictionary<string, object?> { ["message"] = message });
            ChatInputTextBox.Clear();
            await TryRefreshFromBackendAsync(showNotifications: false);

        }
        catch (Exception ex)
        {
            ChatInputTextBox.CaretIndex = ChatInputTextBox.Text.Length;
            if (!await TryRecoverBackendAsync(ex, "Send failed"))
            {
                ShowBackendFailure(ex, "Send failed");
            }
        }
        finally
        {
            SendButton.IsEnabled = true;
            ComposerHintTextBlock.Text = "Ask something substantive. The companion will push back instead of ignoring you.";
        }
    }

    private async void PauseAutonomy_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            var client = _backendProcessService.CreateClient();
            if (_autonomyPaused)
            {
                await client.SendAsync("resume_autonomy", new Dictionary<string, object?>());
                _autonomyPaused = false;
                AutonomyButton.Content = "Pause Autonomy";
            }
            else
            {
                await client.SendAsync("pause_autonomy", new Dictionary<string, object?>());
                _autonomyPaused = true;
                AutonomyButton.Content = "Resume Autonomy";
            }
            await TryRefreshFromBackendAsync(showNotifications: false);
        }
        catch (Exception ex)
        {
            if (!await TryRecoverBackendAsync(ex, "Autonomy update failed"))
            {
                ShowBackendFailure(ex, "Autonomy update failed");
            }
        }
    }

    private async void Settings_Click(object sender, RoutedEventArgs e)
    {
        var previousSettings = _settings.Clone();
        var dialog = new SettingsWindow(_settingsStore, _settings) { Owner = this };
        if (dialog.ShowDialog() != true)
        {
            return;
        }

        var proposedSettings = dialog.Settings.Clone();

        var requiresRestart =
            !string.Equals(previousSettings.ModelPath, proposedSettings.ModelPath, StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(previousSettings.PersonaPath, proposedSettings.PersonaPath, StringComparison.OrdinalIgnoreCase) ||
            previousSettings.ContextSize != proposedSettings.ContextSize ||
            previousSettings.GpuLayers != proposedSettings.GpuLayers;

        _settings.ApplyFrom(proposedSettings);

        try
        {
            if (requiresRestart)
            {
                await _backendProcessService.RestartAsync();
            }

            var client = _backendProcessService.CreateClient();
            await client.SendAsync(
                "update_settings",
                new Dictionary<string, object?>
                {
                    ["autonomy_enabled"] = _settings.AutonomyEnabled,
                    ["quiet_mode"] = _settings.QuietMode,
                    ["model_path"] = _settings.ModelPath,
                    ["persona_path"] = _settings.PersonaPath,
                    ["n_ctx"] = _settings.ContextSize,
                    ["n_gpu_layers"] = _settings.GpuLayers,
                });

            await _settingsStore.SaveAsync(_settings);
            if (_settings.StartAtLogin)
            {
                _startupRegistrationService.Enable();
            }
            else
            {
                _startupRegistrationService.Disable();
            }
            QuietModeTextBlock.Text = _settings.QuietMode ? "Quiet mode: on" : "Quiet mode: off";
            ModelPathTextBlock.Text = $"Model: {_settings.ModelPath}";
            await TryRefreshFromBackendAsync(showNotifications: false);
        }
        catch (Exception ex)
        {
            _settings.ApplyFrom(previousSettings);
            if (requiresRestart)
            {
                try
                {
                    await _backendProcessService.RestartAsync();
                }
                catch
                {
                }
            }
            await _settingsStore.SaveAsync(_settings);
            if (_settings.StartAtLogin)
            {
                _startupRegistrationService.Enable();
            }
            else
            {
                _startupRegistrationService.Disable();
            }
            if (!await TryRecoverBackendAsync(ex, "Settings update failed"))
            {
                ShowBackendFailure(ex, "Settings update failed");
            }
        }
    }

    private async Task ToggleQuietModeAsync()
    {
        var previousQuietMode = _settings.QuietMode;
        try
        {
            _settings.QuietMode = !_settings.QuietMode;
            var client = _backendProcessService.CreateClient();
            await client.SendAsync("update_settings", new Dictionary<string, object?> { ["quiet_mode"] = _settings.QuietMode });
            await _settingsStore.SaveAsync(_settings);
            QuietModeTextBlock.Text = _settings.QuietMode ? "Quiet mode: on" : "Quiet mode: off";
        }
        catch (Exception ex)
        {
            _settings.QuietMode = previousQuietMode;
            await _settingsStore.SaveAsync(_settings);
            if (!await TryRecoverBackendAsync(ex, "Quiet mode update failed"))
            {
                ShowBackendFailure(ex, "Quiet mode update failed");
            }
        }
    }

    private async Task ExitApplicationAsync()
    {
        _notifyIcon.Visible = false;
        _pollTimer.Stop();
        _backendWatchdogTimer.Stop();
        _allowClose = true;
        await _backendProcessService.DisposeAsync();
        System.Windows.Application.Current.ShutdownMode = ShutdownMode.OnMainWindowClose;
        Close();
        System.Windows.Application.Current.Shutdown();
    }

    private void MainWindow_StateChanged(object? sender, EventArgs e)
    {
        if (WindowState == WindowState.Minimized)
        {
            Hide();
        }
    }

    private void MainWindow_Closing(object? sender, System.ComponentModel.CancelEventArgs e)
    {
        if (!_allowClose)
        {
            e.Cancel = true;
            Hide();
        }
    }

    private void RestoreFromTray()
    {
        Show();
        WindowState = WindowState.Normal;
        Activate();
    }

    private async void ProbeRuntime_Click(object sender, RoutedEventArgs e)
    {
        RuntimeProbeTextBlock.Text = "Live check: running probe against GGUF runtime...";
        try
        {
            var client = _backendProcessService.CreateClient();
            var response = await client.SendAsync("probe_runtime", new Dictionary<string, object?>());
            ApplyRuntimeProbe(response.Payload);
        }
        catch (Exception ex)
        {
            RuntimeProbeTextBlock.Text = $"Live check: failed to run probe | {ex.Message}";
            if (!await TryRecoverBackendAsync(ex, "Runtime probe failed"))
            {
                ShowBackendFailure(ex, "Runtime probe failed", showDialog: false);
            }
        }
    }

    private void ApplyRuntimeProbe(Dictionary<string, object?> payload)
    {
        var json = JsonSerializer.Serialize(payload);
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;

        var ok = root.TryGetProperty("ok", out var okElement) && okElement.GetBoolean();
        var checkedAt = root.TryGetProperty("checked_at", out var checkedAtElement)
            ? checkedAtElement.GetString()
            : null;
        var detail = root.TryGetProperty("detail", out var detailElement)
            ? detailElement.GetString() ?? "No detail returned."
            : "No detail returned.";
        var runtimeLabel = root.TryGetProperty("runtime_label", out var runtimeLabelElement)
            ? runtimeLabelElement.GetString() ?? "Unknown runtime"
            : "Unknown runtime";
        var runtimeError = root.TryGetProperty("runtime_error", out var runtimeErrorElement)
            ? runtimeErrorElement.GetString()
            : null;

        GgufStatusTextBlock.Text = ok ? "GGUF STATUS: ACTIVE" : "GGUF STATUS: NOT RUNNING";
        RuntimeTextBlock.Text = ok ? "Backend: live GGUF runtime detected" : "Backend: runtime probe failed";
        RuntimeDetailTextBlock.Text = string.IsNullOrWhiteSpace(runtimeError)
            ? runtimeLabel
            : $"{runtimeLabel} | {runtimeError}";
        RuntimeProbeTextBlock.Text = checkedAt is not null
            ? $"Live check: {(ok ? "PASS" : "FAIL")} at {checkedAt} | {detail}"
            : $"Live check: {(ok ? "PASS" : "FAIL")} | {detail}";
    }

    private void ShowBackendFailure(Exception ex, string context, bool showDialog = true)
    {
        RuntimeTextBlock.Text = $"Backend issue: {context}";
        RuntimeDetailTextBlock.Text = ex.Message;
        StatusTextBlock.Text = "Backend disconnected";
        ComposerHintTextBlock.Text = "Backend hiccup detected. Retry in a moment.";
        if (showDialog)
        {
            System.Windows.MessageBox.Show(this, ex.Message, context, MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private async Task<bool> TryRecoverBackendAsync(Exception ex, string context)
    {
        if (_isRecoveringBackend || !LooksLikeBackendConnectionFailure(ex))
        {
            return false;
        }

        _isRecoveringBackend = true;
        try
        {
            RuntimeTextBlock.Text = $"Backend issue: {context}";
            RuntimeDetailTextBlock.Text = "Trying to restart backend...";
            StatusTextBlock.Text = "Recovering backend";
            await _backendProcessService.RestartAsync();
            await RefreshFromBackendAsync(showNotifications: false);
            ComposerHintTextBlock.Text = "Backend recovered. You can keep going.";
            return true;
        }
        catch
        {
            return false;
        }
        finally
        {
            _isRecoveringBackend = false;
        }
    }

    private static bool LooksLikeBackendConnectionFailure(Exception ex)
    {
        return ex is IOException
            || ex is TimeoutException
            || ex.Message.Contains("pipe", StringComparison.OrdinalIgnoreCase)
            || ex.Message.Contains("backend", StringComparison.OrdinalIgnoreCase)
            || ex.Message.Contains("semaphore timeout", StringComparison.OrdinalIgnoreCase);
    }

    private async Task EnsureBackendProcessAsync()
    {
        if (_isRecoveringBackend || _backendProcessService.IsBackendProcessRunning)
        {
            return;
        }

        _isRecoveringBackend = true;
        try
        {
            RuntimeTextBlock.Text = "Backend issue: process stopped";
            RuntimeDetailTextBlock.Text = "Restarting backend process...";
            StatusTextBlock.Text = "Recovering backend";
            await _backendProcessService.StartAsync();
            await RefreshFromBackendAsync(showNotifications: false);
            ComposerHintTextBlock.Text = "Backend recovered. You can keep going.";
        }
        catch (Exception ex)
        {
            ShowBackendFailure(ex, "Backend process restart failed", showDialog: false);
        }
        finally
        {
            _isRecoveringBackend = false;
        }
    }

    private void BackendProcessService_BackendExited(object? sender, EventArgs e)
    {
        _ = Dispatcher.BeginInvoke(async () =>
        {
            if (_allowClose)
            {
                return;
            }

            await EnsureBackendProcessAsync();
        });
    }
}
