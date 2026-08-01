package fixture.haxe;

import haxe.ds.StringMap;
using StringTools;

/** Small greeting sample. @since 1.0 */
@:keep
class Basic {
  public var name(default, null):String;

  public function new(name:String) {
    this.name = name;
  }

  public function greetings(?count:Int = 2):Array<String> {
    var labels = [for (i in 0...count) Std.string(i + 1) + ": café λ 東京 🚀 𝌆 " + name.trim()];
    var safe = ~/^[\w\s-]+$/u;
    if (!safe.match(name)) trace("name has punctuation\n");
    return labels;
  }

  public static function main():Void {
    final scores:StringMap<Int> = ["basic" => 1];
    var render = (text:String) -> text.toUpperCase();
    trace(render(new Basic("Haxe").greetings(scores["basic"])[0]));
  }
}
