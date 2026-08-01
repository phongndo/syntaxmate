package fixtures.basic {
    import flash.display.Sprite;

    /** Multiline ASDoc:
     *  café λ 東京 🚀 𝌆
     *  @see Sprite
     */
    [Event(name="ready", type="flash.events.Event")]
    public final class Basic extends Sprite {
        private const LABEL:String = "café λ 東京 🚀 𝌆";
        private var count:int = 0x2A;

        public function Basic(name:String = "world") {
            var pattern:RegExp = /café|東京/gi;
            var values:Vector.<int> = new <int>[1, 2, 3];
            if (pattern.test(name) && values.length > 0) {
                trace("Hello, " + name + " 🚀");
            } else {
                trace('fallback\n𝌆');
            }
        }
    }
}
