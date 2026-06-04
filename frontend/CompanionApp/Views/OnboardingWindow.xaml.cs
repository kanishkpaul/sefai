using System.Windows;
using CompanionApp.Models;
using CompanionApp.Services;

namespace CompanionApp.Views;

public partial class OnboardingWindow : Window
{
    private readonly SettingsStore _settingsStore;
    private readonly AppSettings _settings;

    public OnboardingWindow(SettingsStore settingsStore, AppSettings settings)
    {
        InitializeComponent();
        _settingsStore = settingsStore;
        _settings = settings;

        ModelPathTextBox.Text = _settings.ModelPath;
        PersonaPathTextBox.Text = _settings.PersonaPath;
        AutonomyCheckBox.IsChecked = _settings.AutonomyEnabled;
        StartupCheckBox.IsChecked = _settings.StartAtLogin;
    }

    private async void SaveAndContinue_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            _settings.ModelPath = ModelPathTextBox.Text.Trim();
            _settings.PersonaPath = PersonaPathTextBox.Text.Trim();
            _settings.AutonomyEnabled = AutonomyCheckBox.IsChecked == true;
            _settings.StartAtLogin = StartupCheckBox.IsChecked == true;
            await _settingsStore.SaveAsync(_settings);
            DialogResult = true;
            Close();
        }
        catch (Exception ex)
        {
            System.Windows.MessageBox.Show(this, ex.Message, "Failed to save onboarding", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}
