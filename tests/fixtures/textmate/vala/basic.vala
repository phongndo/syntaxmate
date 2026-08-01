using GLib;

/* Multiline café λ 東京
 * remains a comment until this close: 🚀 𝌆. */
public class Greeter : Object {
    private string name = "world";
    public const int MAX_COUNT = 0x2A;

    public string welcome (int count = 3) {
        var decorated = @"Hello $name: $(count + 1)";
        var poem = """café λ
東京 🚀 𝌆""";
        char initial = '\u03bb';
        if (count > 0 and name != null) {
            return "%s — %s".printf (decorated, poem);
        }
        return "none";
    }
}

int main (string[] args) {
    // Function calls, primitive types, constants, and variables.
    var greeter = new Greeter ();
    stdout.printf ("%s\n", greeter.welcome (args.length));
    return 0;
}
