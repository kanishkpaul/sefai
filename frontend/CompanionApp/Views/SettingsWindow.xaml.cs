using System.Windows;
using CompanionApp.Models;
using CompanionApp.Services;

namespace CompanionApp.Views;

public partial class SettingsWindow : Window
{
    private readonly SettingsStore _settingsStore;
    private readonly AppSettings _workingCopy;
    public AppSettings Settings { get; }

    public SettingsWindow(SettingsStore settingsStore, AppSettings settings)
    {
        InitializeComponent();
        _settingsStore = settingsStore;
        _workingCopy = settings.Clone();
        Settings = _workingCopy;

        ModelPathTextBox.Text = _workingCopy.ModelPath;
        PersonaPathTextBox.Text = _workingCopy.PersonaPath;
        ContextSizeTextBox.Text = _workingCopy.ContextSize.ToString();
        GpuLayersTextBox.Text = _workingCopy.GpuLayers.ToString();
        AutonomyCheckBox.IsChecked = _workingCopy.AutonomyEnabled;
        StartupCheckBox.IsChecked = _workingCopy.StartAtLogin;
        NotificationsCheckBox.IsChecked = _workingCopy.NotificationsEnabled;
        QuietModeCheckBox.IsChecked = _workingCopy.QuietMode;
    }

    private async void Save_Click(object sender, RoutedEventArgs e)
    {
        try
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
        catch (Exception ex)
        {
            System.Windows.MessageBox.Show(this, ex.Message, "Failed to save settings", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}
