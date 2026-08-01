#define TRACE_FIXTURE
#pragma warning disable 0168, 0219
#region Imports
global using System;
global using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Text = System.String;
using static System.Math;
#endregion
[assembly: CLSCompliant(true)]
namespace Mark.Syntax.Stress
{
    /// <summary>Models café data &amp; launches a <c>🚀</c>.</summary>
    /// <typeparam name="T">A comparable payload.</typeparam>
    public delegate TResult Projector<in T, out TResult>(T value) where T : notnull;
    [Flags]
    public enum Mode : ushort
    {
        None = 0,
        Read = 0b0001,
        Write = 0x_02,
        All = Read | Write,
    }
    public interface IStore<T> where T : class
    {
        event EventHandler? Changed;
        T? Get(int index);
        void Set(int index, T? value);
        Task<T?> FindAsync(string key, int limit = 10);
    }
    public readonly record struct Coordinate(double X, double Y)
    {
        public double Length => Sqrt(X * X + Y * Y);
    }
    public record Packet<T>(string Id, T Payload)
    {
        public required DateTimeOffset Created { get; init; }
    }
    [Serializable]
    public sealed class MemoryStore<T> : IStore<T> where T : class, IComparable<T>, new()
    {
        private readonly List<T?> _items = new();
        private const decimal Tax = 0.075m;
        private volatile bool _busy;
        public event EventHandler? Changed;

        public MemoryStore(int capacity = 4) : this(new List<T?>(capacity)) { }
        private MemoryStore(List<T?> seed)
        {
            _items = seed ?? throw new ArgumentNullException(nameof(seed));
        }
        ~MemoryStore() => Console.Error.WriteLine("released");
        public T? Current
        {
            get => _items.Count > 0 ? _items[^1] : null;
            private set { if (value is not null) _items.Add(value); }
        }
        public T? Get(int index) => index >= 0 && index < _items.Count ? _items[index] : null;
        public void Set(int index, T? value) => _items[index] = value;

        public void Add(T item)
        {
            _items.Add(item);
        }
        public int Count() => _items.Count;
        public T?[] ToArray() => _items.ToArray();
        public async Task<T?> FindAsync(string key, int limit = 10)
        {
            await Task.Yield();
            var normalized = key?.Trim() ?? string.Empty;
            return _items.Take(limit).FirstOrDefault(x => x?.ToString() == normalized);
        }
        public IEnumerable<string> Describe()
        {
            var query = _items
                .Select(item => item?.ToString() ?? "∅")
                .Where(text => text.Length > 0)
                .OrderByDescending(text => text.Length)
                .ThenBy(text => text);
            foreach (var text in query)
                yield return text;
            yield break;
        }
        public int Classify(object? candidate)
        {
            if (candidate is null) return -1;
            if (candidate is string { Length: 0 }) return 0;
            if (candidate is string { Length: > 0 and < 8 } shortText && shortText != "skip") return 1;
            if (candidate is int and >= 0) return 2;
            if (candidate is int[] { Length: > 2 } values && values[0] == 1) return values.Length;
            if (candidate is Coordinate(0, 0)) return 3;
            if (candidate is Coordinate { X: > 0, Y: <= 0 }) return 4;
            return 99;
        }
        public void Mutate(object gate, params T[] additions)
        {
            const int Mask = 1_000_000;
            var numbers = new[] { 0xCA_FE, 0b1010_0101, 6.022e23, .5f };
            var tuple = (Name: "naïve λ", Count: additions.Length);
            (var name, var count) = tuple;
            (name, count) = (name.ToUpperInvariant(), count + 1);
            Func<int, int> square = static value => value * value;
            Predicate<T> valid = delegate (T value) { return value.CompareTo(new T()) >= 0; };
            var projection = additions.Select((item, index) => new { item, index });

            lock (gate)
            {
                _busy = true;
                for (var i = 0; i < additions.Length; i++)
                {
                    if (valid(additions[i])) _items.Add(additions[i]);
                    else continue;
                }
                foreach (var (item, index) in projection)
                    Console.WriteLine($"[{index,3:X}] {item!} — {square(index)}");
                _busy = false;
            }

            do { count--; } while (count > Mask);
            while (count++ < 2) { if (count == 1) break; }
        }

        public string Render(Packet<T> packet)
        {
            var escaped = "tab:\t quote:\" rocket:\U0001F680";
            var verbatim = @"C:\fixtures\café\""quoted""";
            var interpolated = $@"id={packet.Id}; payload=""{packet.Payload}""";
            var raw = """
                { "kind": "literal", "value": "λ" }
                """;
            var rawInterpolation = $$"""
                { "id": "{{packet.Id}}", "braces": "{kept}" }
                """;
            return $"{escaped}|{verbatim}|{interpolated}|{raw}|{rawInterpolation}";
        }

        public async Task<int> ControlFlowAsync(object input)
        {
        retry:
            try
            {
                using var timer = new System.Threading.PeriodicTimer(TimeSpan.FromMilliseconds(1));
                if (input is not string text) throw new InvalidOperationException("expected text");
                await using var stream = new System.IO.MemoryStream();
                checked { stream.WriteByte((byte)text.Length); }
                return await LocalAsync(text, stream.Length);
            }
            catch (InvalidOperationException error) when (error.Message.Contains("expected"))
            {
                input = string.Empty;
                goto retry;
            }
            finally
            {
                Changed?.Invoke(this, EventArgs.Empty);
            }

            static async Task<int> LocalAsync(string value, long offset)
            {
                await Task.Delay(1).ConfigureAwait(false);
                return unchecked((int)offset) + value.Length;
            }
        }

        public unsafe int PointerSample(int[] values)
        {
            fixed (int* pointer = values)
            {
                int* cursor = pointer;
                return cursor == null ? sizeof(int) : *cursor;
            }
        }
    }

    public static class EntryPoint
    {
        public static void Main()
        {
            var store = new MemoryStore<string>();
            store.Add("delta");
            var clone = new Packet<string>("π-🚀", "payload") { Created = DateTimeOffset.UtcNow };
            Console.WriteLine(store.Render(clone with { Id = "copy" }));
        }
    }
}
#pragma warning restore 0168, 0219
