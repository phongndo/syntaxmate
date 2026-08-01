package fixtures.stress {
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.utils.Dictionary;
    import flash.utils.getTimer;

    use namespace fixture_internal;
    public namespace fixture_internal = "urn:fixture:internal";

    /**
     * Reviewable ActionScript 3 grammar stress source.
     * Unicode coverage: café λ 東京 🚀 𝌆.
     * @eventType flash.events.Event.COMPLETE
     * @see flash.display.Sprite
     */
    public interface Renderable {
        function render(prefix:String = "item"):String;
        function get size():int;
    }

    [Event(name="complete", type="flash.events.Event")]
    [Bindable(event="complete")]
    public dynamic class Catalog extends Sprite implements Renderable {
        private static const MAX_ITEMS:uint = 100;
        private static const EPSILON:Number = 1.0e-6;
        private const names:Vector.<String> = new <String>[];
        private const scores:Dictionary = new Dictionary();
        private var _size:int = 0;
        private var startedAt:Number = NaN;
        fixture_internal var debugLabel:String = @"verbatim café λ 東京 🚀 𝌆";

        /**
         * Creates a catalog and preserves a multiline lexical comment state.
         * @param seed initial names
         */
        public function Catalog(seed:Array = null) {
            super();
            startedAt = getTimer();
            if (seed != null) {
                for each (var raw:Object in seed) {
                    add(String(raw));
                }
            }
        }

        public function add(name:String, score:Number = 0):Boolean {
            var clean:String = normalize(name);
            if (clean.length == 0 || names.length >= MAX_ITEMS) {
                return false;
            }
            names.push(clean);
            scores[clean] = score;
            _size++;
            dispatchEvent(new Event(Event.CHANGE));
            return true;
        }

        public function remove(name:String):Boolean {
            var index:int = names.indexOf(name);
            if (index < 0) {
                return false;
            }
            names.splice(index, 1);
            delete scores[name];
            _size--;
            return true;
        }

        public function get size():int {
            return _size;
        }

        public function set title(value:String):void {
            debugLabel = value == null ? "" : value;
        }

        fixture_internal function elapsed():Number {
            return getTimer() - startedAt;
        }

        public function render(prefix:String = "item"):String {
            var output:Array = [];
            var matcher:RegExp = /^(café|東京|rocket)(?:\s+.*)?$/i;
            for (var i:int = 0; i < names.length; i++) {
                var label:String = names[i];
                var marker:String = matcher.test(label) ? "✓" : "·";
                output.push(prefix + "[" + i + "] " + marker + " " + label);
            }
            return output.join("\n");
        }

        public function summarize(...values):Object {
            var result:Object = {
                count: 0,
                total: 0,
                minimum: Infinity,
                maximum: -Infinity,
                unicode: "café λ 東京 🚀 𝌆",
                nested: { valid: true, note: 'closed' }
            };
            for each (var value:* in values) {
                var number:Number = Number(value);
                if (isNaN(number)) {
                    continue;
                }
                result.count += 1;
                result.total += number;
                result.minimum = Math.min(result.minimum, number);
                result.maximum = Math.max(result.maximum, number);
            }
            return result;
        }

        public function classify(value:*):String {
            switch (typeof value) {
                case "string":
                    return value is String ? "text" : "string-like";
                case "number":
                    return value as Number > 0 ? "positive" : "non-positive";
                case "xml":
                    return "markup";
                default:
                    return value === null ? "null" : "other";
            }
        }

        public function guardedLookup(key:String):Number {
            var answer:Number = 0;
            try {
                if (!(key in scores)) {
                    throw new ArgumentError("Unknown key: " + key);
                }
                answer = Number(scores[key]);
            } catch (error:ArgumentError) {
                trace(error.message);
                answer = NaN;
            } finally {
                trace("lookup complete");
            }
            return answer;
        }

        public function countDown(from:int):Vector.<int> {
            var result:Vector.<int> = new Vector.<int>();
            do {
                result.push(from--);
            } while (from >= 0);
            return result;
        }

        public function matrix(rows:uint, columns:uint):Array {
            var grid:Array = [];
            outer: for (var row:uint = 0; row < rows; row++) {
                var cells:Array = [];
                for (var column:uint = 0; column < columns; column++) {
                    if (column > 8) {
                        break;
                    }
                    cells[column] = row * columns + column;
                }
                grid[row] = cells;
            }
            return grid;
        }

        public function parseSamples():Array {
            var hex:uint = 0xCAFE;
            var fraction:Number = .125;
            var exponent:Number = 6.022e+23;
            var escaped:String = "tab:\t quote:\" slash:\\ rocket:🚀";
            var single:String = 'Tokyo 東京 and astral 𝌆';
            var expression:RegExp = /[A-Z_][\w$]*(?:\.[A-Z_][\w$]*)*/g;
            return [hex, fraction, exponent, escaped, single, expression];
        }

        CONFIG::debug function dumpState():void {
            trace(JSON.stringify({ names: names, size: _size }));
        }

        override public function toString():String {
            return "[Catalog size=" + _size + ", elapsed=" + elapsed() + "]";
        }

        private static function normalize(value:String):String {
            if (value == null) {
                return "";
            }
            return value.replace(/^\s+|\s+$/g, "").replace(/\s{2,}/g, " ");
        }
    }
}
