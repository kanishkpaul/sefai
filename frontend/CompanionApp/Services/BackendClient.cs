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
        await pipe.ConnectAsync(5000);
        using var writer = new StreamWriter(pipe, new UTF8Encoding(false), 4096, leaveOpen: true) { AutoFlush = true };
        using var reader = new StreamReader(pipe, Encoding.UTF8, detectEncodingFromByteOrderMarks: false, bufferSize: 4096, leaveOpen: true);

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
