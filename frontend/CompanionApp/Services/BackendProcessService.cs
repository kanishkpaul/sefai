using System.IO;
using System.Diagnostics;
using CompanionApp.Models;

namespace CompanionApp.Services;

public class BackendProcessService : IAsyncDisposable
{
    private readonly AppSettings _settings;
    private Process? _process;
    private readonly string _workingDirectory;
    private readonly string _backendEntryPoint;
    private readonly LlamaRuntimeProvisioner _llamaRuntimeProvisioner;

    public event EventHandler? BackendExited;

    public BackendProcessService(AppSettings settings)
    {
        _settings = settings;
        _workingDirectory = ResolveRepositoryRoot();
        _backendEntryPoint = Path.Combine(_workingDirectory, "backend", "main.py");
        _llamaRuntimeProvisioner = new LlamaRuntimeProvisioner(_workingDirectory);
    }

    public BackendClient CreateClient() => new(_settings.PipeName);

    public bool IsBackendProcessRunning
    {
        get
        {
            if (_process is null)
            {
                return false;
            }

            try
            {
                _process.Refresh();
                return !_process.HasExited;
            }
            catch
            {
                return false;
            }
        }
    }

    public async Task StartAsync()
    {
        if (_process is { HasExited: false })
        {
            return;
        }

        if (!File.Exists(_backendEntryPoint))
        {
            throw new FileNotFoundException($"Backend entry point was not found at {_backendEntryPoint}.");
        }

        await _llamaRuntimeProvisioner.EnsureInstalledAsync();

        var startInfo = new ProcessStartInfo
        {
            FileName = "python",
            Arguments = $"\"{_backendEntryPoint}\"",
            WorkingDirectory = _workingDirectory,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };

        startInfo.Environment["PYTHONUNBUFFERED"] = "1";
        startInfo.Environment["SEFAI_MODEL_PATH"] = _settings.ModelPath;
        startInfo.Environment["SEFAI_PERSONA_PATH"] = _settings.PersonaPath;
        startInfo.Environment["SEFAI_DATABASE_PATH"] = _settings.DatabasePath;
        startInfo.Environment["SEFAI_PIPE_NAME"] = _settings.PipeName;
        startInfo.Environment["SEFAI_LLAMA_CLI_PATH"] = _llamaRuntimeProvisioner.LlamaCliPath;
        startInfo.Environment["SEFAI_PREFER_LLAMA_CLI"] = "true";
        startInfo.Environment["SEFAI_N_CTX"] = _settings.ContextSize.ToString();
        startInfo.Environment["SEFAI_N_THREADS"] = _settings.ThreadCount.ToString();
        startInfo.Environment["SEFAI_N_GPU_LAYERS"] = _settings.GpuLayers.ToString();
        startInfo.Environment["SEFAI_TEMPERATURE"] = _settings.Temperature.ToString(System.Globalization.CultureInfo.InvariantCulture);
        startInfo.Environment["SEFAI_TOP_P"] = _settings.TopP.ToString(System.Globalization.CultureInfo.InvariantCulture);
        startInfo.Environment["SEFAI_AUTONOMY_ENABLED"] = _settings.AutonomyEnabled.ToString().ToLowerInvariant();
        startInfo.Environment["SEFAI_START_AT_LOGIN"] = _settings.StartAtLogin.ToString().ToLowerInvariant();
        startInfo.Environment["SEFAI_NOTIFICATIONS_ENABLED"] = _settings.NotificationsEnabled.ToString().ToLowerInvariant();
        startInfo.Environment["SEFAI_QUIET_MODE"] = _settings.QuietMode.ToString().ToLowerInvariant();
        _process = Process.Start(startInfo) ?? throw new InvalidOperationException("Failed to start backend process.");
        _process.EnableRaisingEvents = true;
        _process.Exited += HandleBackendExited;
        _process.OutputDataReceived += (_, _) => { };
        _process.ErrorDataReceived += (_, _) => { };
        _process.BeginOutputReadLine();
        _process.BeginErrorReadLine();

        await WaitForHealthyAsync();
    }

    public async Task RestartAsync()
    {
        await DisposeAsync();
        await StartAsync();
    }

    public async Task WaitForHealthyAsync()
    {
        var client = CreateClient();
        var started = DateTime.UtcNow;
        Exception? lastError = null;

        while (DateTime.UtcNow - started < TimeSpan.FromSeconds(15))
        {
            try
            {
                var response = await client.SendAsync("health_ping", new Dictionary<string, object?>());
                if (response.Type == "health_pong")
                {
                    return;
                }
            }
            catch (Exception ex)
            {
                lastError = ex;
                await Task.Delay(500);
            }
        }

        throw new InvalidOperationException("Backend did not become healthy.", lastError);
    }

    public async ValueTask DisposeAsync()
    {
        try
        {
            var client = CreateClient();
            await client.SendAsync("shutdown", new Dictionary<string, object?>());
        }
        catch
        {
        }

        if (_process is { HasExited: false })
        {
            _process.Kill(entireProcessTree: true);
            await _process.WaitForExitAsync();
        }
    }

    private void HandleBackendExited(object? sender, EventArgs e)
    {
        BackendExited?.Invoke(this, EventArgs.Empty);
    }

    private static string ResolveRepositoryRoot()
    {
        var current = new DirectoryInfo(AppContext.BaseDirectory);
        while (current is not null)
        {
            if (Directory.Exists(Path.Combine(current.FullName, "backend")) &&
                Directory.Exists(Path.Combine(current.FullName, "frontend")))
            {
                return current.FullName;
            }

            current = current.Parent;
        }

        throw new DirectoryNotFoundException("Could not locate the repository root containing backend/ and frontend/.");
    }
}
