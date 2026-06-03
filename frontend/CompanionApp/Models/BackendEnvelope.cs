using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace CompanionApp.Models;

public class BackendEnvelope
{
    [JsonPropertyName("type")]
    public string Type { get; set; } = "";

    [JsonPropertyName("request_id")]
    public string? RequestId { get; set; }

    [JsonPropertyName("payload")]
    public Dictionary<string, object?> Payload { get; set; } = new();

    [JsonPropertyName("error")]
    public string? Error { get; set; }
}
