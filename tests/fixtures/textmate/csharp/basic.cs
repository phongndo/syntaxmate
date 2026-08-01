using System;
using System.Collections.Generic;

namespace Fixtures.CSharp;

public sealed record Launch(string Name, IReadOnlyList<int> Codes)
{
    /* Multiline state:
       café, λ, 🚀, and 𝌆 stay inside a closed comment. */
    public string Describe(int count)
    {
        var path = @"C:\café\λ\launch.txt";
        var message = $"{Name} sends 🚀 number {count:D2}";
        var document = $$"""
            {
              "symbol": "𝌆",
              "message": "{{message}}"
            }
            """;
        return count > 0 ? $"{path}: {document}" : "idle";
    }
}
