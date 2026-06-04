using System.Windows;
using CompanionApp.Models;
using System.Globalization;

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
        ThreadCountTextBox.Text = _workingCopy.ThreadCount.ToString();
        GpuLayersTextBox.Text = _workingCopy.GpuLayers.ToString();
        TemperatureTextBox.Text = _workingCopy.Temperature.ToString(CultureInfo.InvariantCulture);
        TopPTextBox.Text = _workingCopy.TopP.ToString(CultureInfo.InvariantCulture);
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
            Settings.ThreadCount = ParsePositiveInt(ThreadCountTextBox.Text, "Thread count");
            Settings.GpuLayers = ParseNonNegativeInt(GpuLayersTextBox.Text, "GPU layers");
            Settings.Temperature = ParseNonNegativeDouble(TemperatureTextBox.Text, "Temperature");
            Settings.TopP = ParseProbability(TopPTextBox.Text, "Top P");
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

    private static double ParseNonNegativeDouble(string rawValue, string fieldName)
    {
        if (!double.TryParse(rawValue, NumberStyles.Float | NumberStyles.AllowThousands, CultureInfo.InvariantCulture, out var value) || value < 0)
        {
            throw new InvalidOperationException($"{fieldName} must be a number that is 0 or greater.");
        }

        return value;
    }

    private static double ParseProbability(string rawValue, string fieldName)
    {
        if (!double.TryParse(rawValue, NumberStyles.Float | NumberStyles.AllowThousands, CultureInfo.InvariantCulture, out var value) || value <= 0 || value > 1)
        {
            throw new InvalidOperationException($"{fieldName} must be a number greater than 0 and at most 1.");
        }

        return value;
    }
}
