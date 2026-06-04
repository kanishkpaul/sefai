namespace CompanionApp.Models;

public class AppSettings
{
    public string ModelPath { get; set; } = @"D:\Gemma-4-E2B-Uncensored-HauhauCS-Aggressive-Q4_K_P.gguf";
    public string PersonaPath { get; set; } = System.IO.Path.GetFullPath("persona.sample.json");
    public string DatabasePath { get; set; } = System.IO.Path.GetFullPath(System.IO.Path.Combine("runtime", "companion.db"));
    public string PipeName { get; set; } = "sefai_companion_pipe";
    public int ContextSize { get; set; } = 4096;
    public int ThreadCount { get; set; } = 8;
    public int GpuLayers { get; set; } = 20;
    public double Temperature { get; set; } = 0.72;
    public double TopP { get; set; } = 0.92;
    public bool AutonomyEnabled { get; set; } = true;
    public bool StartAtLogin { get; set; } = true;
    public bool NotificationsEnabled { get; set; } = true;
    public bool QuietMode { get; set; } = false;

    public AppSettings Clone() =>
        new()
        {
            ModelPath = ModelPath,
            PersonaPath = PersonaPath,
            DatabasePath = DatabasePath,
            PipeName = PipeName,
            ContextSize = ContextSize,
            ThreadCount = ThreadCount,
            GpuLayers = GpuLayers,
            Temperature = Temperature,
            TopP = TopP,
            AutonomyEnabled = AutonomyEnabled,
            StartAtLogin = StartAtLogin,
            NotificationsEnabled = NotificationsEnabled,
            QuietMode = QuietMode,
        };
}
