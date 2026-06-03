using System.Windows;
using CompanionApp.Models;
using CompanionApp.Services;

namespace CompanionApp.Views;

public partial class SettingsWindow : Window
{
    private readonly SettingsStore _settingsStore;
    public AppSettings Settings { get; }

    public SettingsWindow(SettingsStore settingsStore, AppSettings settings)
    {
        InitializeComponent();
        _settingsStore = settingsStore;
        Settings = settings;

        ModelPathTextBox.Text = settings.ModelPath;
        PersonaPathTextBox.Text = settings.PersonaPath;
        ContextSizeTextBox.Text = settings.ContextSize.ToString();
        GpuLayersTextBox.Text = settings.GpuLayers.ToString();
        AutonomyCheckBox.IsChecked = settings.AutonomyEnabled;
        StartupCheckBox.IsChecked = settings.StartAtLogin;
        NotificationsCheckBox.IsChecked = settings.NotificationsEnabled;
        QuietModeCheckBox.IsChecked = settings.QuietMode;
    }

    private async void Save_Click(object sender, RoutedEventArgs e)
    {
        Settings.ModelPath = ModelPathTextBox.Text.Trim();
        Settings.PersonaPath = PersonaPathTextBox.Text.Trim();
        Settings.ContextSize = int.TryParse(ContextSizeTextBox.Text, out var context) ? context : Settings.ContextSize;
        Settings.GpuLayers = int.TryParse(GpuLayersTextBox.Text, out var gpuLayers) ? gpuLayers : Settings.GpuLayers;
        Settings.AutonomyEnabled = AutonomyCheckBox.IsChecked == true;
        Settings.StartAtLogin = StartupCheckBox.IsChecked == true;
        Settings.NotificationsEnabled = NotificationsCheckBox.IsChecked == true;
        Settings.QuietMode = QuietModeCheckBox.IsChecked == true;

        await _settingsStore.SaveAsync(Settings);
        DialogResult = true;
        Close();
    }
}
