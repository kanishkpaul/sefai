using System.IO;
using System.Windows;
using CompanionApp.Services;
using CompanionApp.Views;

namespace CompanionApp;

public partial class App : System.Windows.Application
{
    private BackendProcessService? _backendProcessService;
    private StartupRegistrationService? _startupRegistrationService;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        var settingsStore = new SettingsStore();
        var settings = await settingsStore.LoadAsync();
        _backendProcessService = new BackendProcessService(settings);
        _startupRegistrationService = new StartupRegistrationService();

        if (settings.StartAtLogin)
        {
            _startupRegistrationService.Enable();
        }
        else
        {
            _startupRegistrationService.Disable();
        }

        if (!File.Exists(settings.PersonaPath))
        {
            var onboarding = new OnboardingWindow(settingsStore, settings);
            onboarding.ShowDialog();
            settings = await settingsStore.LoadAsync();
            _backendProcessService = new BackendProcessService(settings);
        }

        await _backendProcessService.StartAsync();

        var mainWindow = new MainWindow(settingsStore, settings, _backendProcessService, _startupRegistrationService);
        MainWindow = mainWindow;
        mainWindow.Show();
    }

    protected override async void OnExit(ExitEventArgs e)
    {
        if (_backendProcessService is not null)
        {
            await _backendProcessService.DisposeAsync();
        }
        base.OnExit(e);
    }
}
