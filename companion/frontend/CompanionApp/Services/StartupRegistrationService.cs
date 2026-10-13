using Microsoft.Win32;

namespace CompanionApp.Services;

public class StartupRegistrationService
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string AppName = "SefaiCompanion";

    public void Enable()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true) ?? Registry.CurrentUser.CreateSubKey(RunKeyPath);
        key?.SetValue(AppName, $"\"{Environment.ProcessPath}\"");
    }

    public void Disable()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        key?.DeleteValue(AppName, false);
    }
}
