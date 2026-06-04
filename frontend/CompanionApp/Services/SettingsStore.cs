using System.IO;
using System.Text.Json;
using CompanionApp.Models;

namespace CompanionApp.Services;

public class SettingsStore
{
    private readonly JsonSerializerOptions _jsonOptions = new() { WriteIndented = true };

    public string SettingsPath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "SefaiCompanion",
        "settings.json"
    );

    public async Task<AppSettings> LoadAsync()
    {
        if (!File.Exists(SettingsPath))
        {
            var defaults = new AppSettings();
            defaults.Normalize();
            await SaveAsync(defaults);
            return defaults;
        }

        try
        {
            await using var stream = File.OpenRead(SettingsPath);
            var loaded = await JsonSerializer.DeserializeAsync<AppSettings>(stream, _jsonOptions) ?? new AppSettings();
            loaded.Normalize();
            await SaveAsync(loaded);
            return loaded;
        }
        catch (JsonException)
        {
            var defaults = new AppSettings();
            defaults.Normalize();
            await SaveAsync(defaults);
            return defaults;
        }
    }

    public async Task SaveAsync(AppSettings settings)
    {
        settings.Normalize();
        Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!);
        await using var stream = File.Create(SettingsPath);
        await JsonSerializer.SerializeAsync(stream, settings, _jsonOptions);
    }
}
