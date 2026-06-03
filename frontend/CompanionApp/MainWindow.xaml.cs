using System.Collections.ObjectModel;
using System.Globalization;
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
    private readonly Forms.NotifyIcon _notifyIcon;
    private AppSettings _settings;
    private bool _autonomyPaused;
    private bool _allowClose;
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
        _pollTimer.Tick += async (_, _) => await RefreshFromBackendAsync(showNotifications: true);

        Loaded += async (_, _) =>
        {
            await InitializeFromBackendAsync();
            _pollTimer.Start();
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

        if (root.TryGetProperty("active_goals", out var goals))
        {
            GoalsItemsControl.ItemsSource = goals.EnumerateArray().Select(item => $"• {item.GetString()}").ToArray();
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

        ChatInputTextBox.Clear();
        ComposerHintTextBlock.Text = "Thinking...";
        try
        {
            var client = _backendProcessService.CreateClient();
            var response = await client.SendAsync("send_user_message", new Dictionary<string, object?> { ["message"] = message });
            await RefreshFromBackendAsync(showNotifications: false);

            if (response.Payload.TryGetValue("decision", out var decision) && decision?.ToString() == "ignored")
            {
                _chatMessages.Add(new ChatMessage
                {
                    Speaker = CompanionNameTextBlock.Text,
                    Content = "[Ignored this message]",
                    DeliveryTag = "Ignore",
                    Timestamp = DateTime.Now,
                    AccentBrush = System.Windows.Media.Brushes.Gray,
                });
            }
        }
        catch (Exception ex)
        {
            System.Windows.MessageBox.Show(this, ex.Message, "Send failed", MessageBoxButton.OK, MessageBoxImage.Error);
        }
        finally
        {
            ComposerHintTextBlock.Text = "Ask something substantive. The companion can refuse shallow prompts.";
        }
    }

    private async void PauseAutonomy_Click(object sender, RoutedEventArgs e)
    {
        var client = _backendProcessService.CreateClient();
        var button = (System.Windows.Controls.Button)sender;
        if (_autonomyPaused)
        {
            await client.SendAsync("resume_autonomy", new Dictionary<string, object?>());
            _autonomyPaused = false;
            button.Content = "Pause Autonomy";
        }
        else
        {
            await client.SendAsync("pause_autonomy", new Dictionary<string, object?>());
            _autonomyPaused = true;
            button.Content = "Resume Autonomy";
        }
        await RefreshFromBackendAsync(showNotifications: false);
    }

    private async void Settings_Click(object sender, RoutedEventArgs e)
    {
        var previousModelPath = _settings.ModelPath;
        var previousPersonaPath = _settings.PersonaPath;
        var previousContextSize = _settings.ContextSize;
        var previousGpuLayers = _settings.GpuLayers;
        var dialog = new SettingsWindow(_settingsStore, _settings) { Owner = this };
        if (dialog.ShowDialog() != true)
        {
            return;
        }

        _settings = dialog.Settings;
        await _settingsStore.SaveAsync(_settings);

        if (_settings.StartAtLogin)
        {
            _startupRegistrationService.Enable();
        }
        else
        {
            _startupRegistrationService.Disable();
        }

        var requiresRestart =
            !string.Equals(previousModelPath, _settings.ModelPath, StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(previousPersonaPath, _settings.PersonaPath, StringComparison.OrdinalIgnoreCase) ||
            previousContextSize != _settings.ContextSize ||
            previousGpuLayers != _settings.GpuLayers;

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

        QuietModeTextBlock.Text = _settings.QuietMode ? "Quiet mode: on" : "Quiet mode: off";
        ModelPathTextBlock.Text = $"Model: {_settings.ModelPath}";
        await RefreshFromBackendAsync(showNotifications: false);
    }

    private async Task ToggleQuietModeAsync()
    {
        _settings.QuietMode = !_settings.QuietMode;
        await _settingsStore.SaveAsync(_settings);
        var client = _backendProcessService.CreateClient();
        await client.SendAsync("update_settings", new Dictionary<string, object?> { ["quiet_mode"] = _settings.QuietMode });
        QuietModeTextBlock.Text = _settings.QuietMode ? "Quiet mode: on" : "Quiet mode: off";
    }

    private async Task ExitApplicationAsync()
    {
        _notifyIcon.Visible = false;
        _pollTimer.Stop();
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
}
