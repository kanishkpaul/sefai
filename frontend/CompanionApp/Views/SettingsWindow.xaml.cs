using System.Windows;
using CompanionApp.Models;

namespace CompanionApp.Views;

public partial class SettingsWindow : Window
{
    private readonly AppSettings _workingCopy;
    public AppSettings Settings { get; }

    public SettingsWindow(AppSettings settings)
    {
        InitializeComponent();
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

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            Settings.ModelPath = ModelPathTextBox.Text.Trim();
            Settings.PersonaPath = PersonaPathTextBox.Text.Trim();
            Settings.ContextSize = ParsePositiveInt(ContextSizeTextBox.Text, "Context size");
            Settings.GpuLayers = ParseNonNegativeInt(GpuLayersTextBox.Text, "GPU layers");
            Settings.AutonomyEnabled = AutonomyCheckBox.IsChecked == true;
            Settings.StartAtLogin = StartupCheckBox.IsChecked == true;
            Settings.NotificationsEnabled = NotificationsCheckBox.IsChecked == true;
            Settings.QuietMode = QuietModeCheckBox.IsChecked == true;

            if (!System.IO.File.Exists(Settings.ModelPath))
            {
                throw new InvalidOperationException($"Model file was not found at '{Settings.ModelPath}'.");
            }

            if (!System.IO.File.Exists(Settings.PersonaPath))
            {
                throw new InvalidOperationException($"Persona file was not found at '{Settings.PersonaPath}'.");
            }

            DialogResult = true;
            Close();
        }
        catch (Exception ex)
        {
            System.Windows.MessageBox.Show(this, ex.Message, "Failed to save settings", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }

    private static int ParsePositiveInt(string rawValue, string fieldName)
    {
        if (!int.TryParse(rawValue, out var value) || value <= 0)
        {
            throw new InvalidOperationException($"{fieldName} must be a whole number greater than 0.");
        }

        return value;
    }

    private static int ParseNonNegativeInt(string rawValue, string fieldName)
    {
        if (!int.TryParse(rawValue, out var value) || value < 0)
        {
            throw new InvalidOperationException($"{fieldName} must be a whole number that is 0 or greater.");
        }

        return value;
    }
}
