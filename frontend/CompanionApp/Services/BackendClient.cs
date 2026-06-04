using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Text.Json;
using CompanionApp.Models;

namespace CompanionApp.Services;

public class BackendClient
{
    private readonly string _pipeName;
    private readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web);

    public BackendClient(string pipeName)
    {
        _pipeName = pipeName;
    }

    public async Task<BackendEnvelope> SendAsync(string type, Dictionary<string, object?> payload)
    {
        using var pipe = new NamedPipeClientStream(".", _pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
        var started = DateTime.UtcNow;
        Exception? lastError = null;
        while (!pipe.IsConnected && DateTime.UtcNow - started < TimeSpan.FromSeconds(12))
        {
            try
            {
                await pipe.ConnectAsync(1200);
            }
            catch (TimeoutException ex)
            {
                lastError = ex;
                await Task.Delay(150);
            }
            catch (IOException ex)
            {
                lastError = ex;
                await Task.Delay(150);
            }
        }

        if (!pipe.IsConnected)
        {
            throw new InvalidOperationException("Could not connect to the backend pipe.", lastError);
        }

        var writer = new StreamWriter(pipe, new UTF8Encoding(false), 4096, leaveOpen: true) { AutoFlush = true };
        var reader = new StreamReader(pipe, Encoding.UTF8, detectEncodingFromByteOrderMarks: false, bufferSize: 4096, leaveOpen: true);

        var envelope = new BackendEnvelope
        {
            Type = type,
            RequestId = Guid.NewGuid().ToString("N"),
            Payload = payload,
        };

        var requestJson = JsonSerializer.Serialize(envelope, _jsonOptions);
        await writer.WriteLineAsync(requestJson);
        var responseJson = await reader.ReadLineAsync();
        if (string.IsNullOrWhiteSpace(responseJson))
        {
            throw new InvalidOperationException("Backend returned an empty response.");
        }

        var response = JsonSerializer.Deserialize<BackendEnvelope>(responseJson, _jsonOptions);
        if (response is null)
        {
            throw new InvalidOperationException("Failed to deserialize backend response.");
        }

        return response;
    }
}
