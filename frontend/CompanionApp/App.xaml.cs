using System.IO;
using System.Threading;
using System.Windows;
using CompanionApp.Services;
using CompanionApp.Views;

namespace CompanionApp;

public partial class App : System.Windows.Application
{
    private static Mutex? _singleInstanceMutex;
    private BackendProcessService? _backendProcessService;
    private StartupRegistrationService? _startupRegistrationService;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _singleInstanceMutex = new Mutex(initiallyOwned: true, name: "Local\\SefaiCompanionSingleton", createdNew: out var createdNew);
        if (!createdNew)
        {
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            Shutdown();
            return;
        }

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
        _singleInstanceMutex?.ReleaseMutex();
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
