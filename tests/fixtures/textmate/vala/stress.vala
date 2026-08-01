using GLib;
using Gee;

// Tier-B Vala stress fixture: café λ 東京 🚀 𝌆
/* This block comment opens on one physical line.
 * It carries punctuation, keywords like class and while, and Unicode.
 * The closing delimiter restores ordinary code tokenization. */
/** Documentation-looking text is handled by the imported grammar. */
/**/

#if FIXTURE_MODE
const int BUILD_LEVEL = 7;
#elif DEBUG
const int BUILD_LEVEL = 3;
#else
const int BUILD_LEVEL = 1;
#endif

namespace Oracle.Fixtures {
    public errordomain FixtureError {
        INVALID_INPUT,
        NOT_READY,
        IO_FAILURE
    }

    public enum Phase {
        QUEUED,
        RUNNING,
        COMPLETE
    }

    [Flags]
    public enum Permission {
        READ = 1,
        WRITE = 2,
        EXECUTE = 4
    }

    public struct Point {
        public double x;
        public double y;
    }

    public interface Renderable : Object {
        public abstract string render ();
        public virtual bool visible { get { return true; } }
    }

    public delegate bool Predicate<T> (T value);

    public abstract class Entity : Object, Renderable {
        public uint64 id { get; construct set; }
        public string title { owned get; set; default = "untitled"; }
        public weak Object? owner { get; set; }
        public signal void changed (string field);

        protected Entity (uint64 id) {
            Object (id: id);
        }

        public abstract string render ();
    }

    public sealed class Job : Entity {
        private static int instance_count = 0;
        private const double GOLDEN_RATIO = 1.6180339;
        private volatile bool cancelled = false;
        public Phase phase { get; private set; default = Phase.QUEUED; }
        public GenericArray<string> tags { get; private set; }

        public Job (uint64 id, string title) {
            base (id);
            this.title = title;
            tags = new GenericArray<string> ();
            instance_count++;
        }

        construct {
            tags.add ("created");
        }

        ~Job () {
            instance_count--;
        }

        public override string render () {
            return @"Job $id: $title ($(phase.to_string ()))";
        }

        public void add_tag (owned string tag) requires (tag.length > 0) {
            tags.add ((owned) tag);
            changed ("tags");
        }

        public async string load_async (Cancellable? cancellable = null)
                throws FixtureError, IOError {
            yield Timeout.add_once (1, load_async.callback);
            if (cancelled) {
                throw new FixtureError.NOT_READY ("cancelled");
            }
            return render ();
        }
    }

    internal class LexicalGallery : Object {
        public void numbers () {
            int decimal_value = 1_000;
            uint hex_value = 0xCAFEu;
            long signed_long = 42L;
            ulong unsigned_long = 42UL;
            double floating = 12.50;
            float leading = .75f;
            double exponent = 6.02E+23;
            int8 tiny = -8;
            uint64 huge = 184467u;
            stdout.printf ("%d %u %ld %lu %.2f %.2f %g %d %lu\n",
                           decimal_value, hex_value, signed_long,
                           unsigned_long, floating, leading,
                           exponent, tiny, huge);
        }

        public string strings (string name) {
            string escaped = "quote=\" slash=\\ newline=\n λ";
            string interpolated = @"hello $name: $(name.up ())";
            string nested = @"length=$(name.substring (0, (name.length > 2 ? 2 : name.length)).length)";
            char rune = '\u03bb';
            string triple = """First line café λ
Second line 東京 with "quotes" and $not_interpolated
Third line carries astral 🚀 𝌆 and closes next.""";
            Regex path = /^(?<drive>[A-Z]:)?\/(?:[^\/]+\/)*[^\/]+$/;
            return "%s|%s|%s|%c|%s|%s".printf (
                escaped, interpolated, nested, rune, triple, path.pattern);
        }

        public bool controls (int limit) {
            var values = new ArrayList<int> ();
            for (int index = 0; index < limit; index++) {
                values.add (index);
            }

            foreach (var value in values) {
                if (value == 0) {
                    continue;
                } else if (value > 20) {
                    break;
                }

                switch (value) {
                case 1:
                    stdout.puts ("one\n");
                    break;
                case 2:
                    stdout.puts ("two\n");
                    break;
                default:
                    stdout.printf ("%d\n", value);
                    break;
                }
            }

            int attempts = 0;
            do {
                attempts++;
            } while (attempts < 2);

            try {
                lock (values) {
                    return values.size > 0 and not cancelled_value ();
                }
            } catch (Error error) {
                warning ("caught: %s", error.message);
            } finally {
                unlock (values);
            }
            return false;
        }

        private bool cancelled_value () {
            bool? optional = null;
            var result = optional ?? false;
            return result;
        }

        public void keyword_gallery (ref int input, out int output) {
            output = input;
            var boxed = input as Object;
            bool check = boxed is Object;
            Type type = typeof (Job);
            size_t width = sizeof (uint64);
            unowned string borrowed = "borrowed";
            dynamic Object dynamic_value = new Object ();
            delete boxed;
            stdout.printf ("%s %s %lu %s %s\n", check.to_string (),
                           type.name (), width, borrowed,
                           dynamic_value.get_type ().name ());
        }
    }
}

public static int main (string[] args) {
    var gallery = new Oracle.Fixtures.LexicalGallery ();
    gallery.numbers ();
    stdout.printf ("%s\n", gallery.strings ("café λ 東京 🚀 𝌆"));
    int input = args.length;
    int output;
    gallery.keyword_gallery (ref input, out output);
    return gallery.controls (output + 3) ? 0 : 1;
}
