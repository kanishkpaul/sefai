using System.IO;
using System.Threading;
using System.Windows;
using CompanionApp.Services;
using CompanionApp.Views;

namespace CompanionApp;

public partial class App : System.Windows.Application
{
    private static Mutex? _singleInstanceMutex;
    private static bool _ownsSingleInstanceMutex;
    private BackendProcessService? _backendProcessService;
    private StartupRegistrationService? _startupRegistrationService;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        try
        {
            _singleInstanceMutex = new Mutex(initiallyOwned: true, name: "Local\\SefaiCompanionSingleton", createdNew: out var createdNew);
            _ownsSingleInstanceMutex = createdNew;
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
                if (onboarding.ShowDialog() != true)
                {
                    ShutdownMode = ShutdownMode.OnExplicitShutdown;
                    Shutdown();
                    return;
                }
                settings = await settingsStore.LoadAsync();
                if (!File.Exists(settings.PersonaPath) || !File.Exists(settings.ModelPath))
                {
                    throw new InvalidOperationException("Onboarding did not produce valid local model/persona paths.");
                }
                _backendProcessService = new BackendProcessService(settings);
            }

            await _backendProcessService.StartAsync();

            var mainWindow = new MainWindow(settingsStore, settings, _backendProcessService, _startupRegistrationService);
            MainWindow = mainWindow;
            mainWindow.Show();
        }
        catch (Exception ex)
        {
            System.Windows.MessageBox.Show(
                ex.Message,
                "Sefai Companion failed to start",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            Shutdown();
        }
    }

    protected override async void OnExit(ExitEventArgs e)
    {
        try
        {
            if (_backendProcessService is not null)
            {
                await _backendProcessService.DisposeAsync();
            }
        }
        catch
        {
        }
        if (_ownsSingleInstanceMutex)
        {
            _singleInstanceMutex?.ReleaseMutex();
        }
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
