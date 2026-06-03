using System;

namespace CompanionApp.Models;

public class ChatMessage
{
    public string Speaker { get; set; } = "";
    public string Content { get; set; } = "";
    public string DeliveryTag { get; set; } = "";
    public DateTime Timestamp { get; set; } = DateTime.Now;
    public System.Windows.Media.Brush AccentBrush { get; set; } = System.Windows.Media.Brushes.Black;
}
