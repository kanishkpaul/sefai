using System.IO;
using System.IO.Compression;
using System.Net.Http;

namespace CompanionApp.Services;

public class LlamaRuntimeProvisioner
{
    private const string LlamaVersion = "b9490";
    private const string RuntimeArchiveName = "llama-b9490-bin-win-cpu-x64.zip";
    private const string DownloadUrl = "https://github.com/ggml-org/llama.cpp/releases/download/b9490/llama-b9490-bin-win-cpu-x64.zip";

    private readonly string _repoRoot;
    private readonly string _runtimeDir;
    private readonly string _extractDir;
    private readonly string _archivePath;

    public LlamaRuntimeProvisioner(string repoRoot)
    {
        _repoRoot = repoRoot;
        _runtimeDir = Path.Combine(repoRoot, "runtime_tools", "llama_cpp");
        _extractDir = Path.Combine(_runtimeDir, LlamaVersion);
        _archivePath = Path.Combine(_runtimeDir, RuntimeArchiveName);
    }

    public string LlamaCliPath => Path.Combine(_extractDir, "llama-cli.exe");

    public async Task EnsureInstalledAsync()
    {
        if (File.Exists(LlamaCliPath))
        {
            return;
        }

        Directory.CreateDirectory(_runtimeDir);

        using var httpClient = new HttpClient();
        await using (var remoteStream = await httpClient.GetStreamAsync(DownloadUrl))
        await using (var localStream = File.Create(_archivePath))
        {
            await remoteStream.CopyToAsync(localStream);
        }

        if (Directory.Exists(_extractDir))
        {
            Directory.Delete(_extractDir, recursive: true);
        }

        ZipFile.ExtractToDirectory(_archivePath, _extractDir, overwriteFiles: true);

        if (!File.Exists(LlamaCliPath))
        {
            throw new FileNotFoundException($"Downloaded llama.cpp runtime, but {LlamaCliPath} was not found after extraction.");
        }
    }
}
