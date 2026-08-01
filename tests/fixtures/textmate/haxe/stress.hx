package fixture.haxe;

import haxe.Json as JsonCodec;
import haxe.ds.IntMap;
import haxe.ds.StringMap;
import sys.io.File;
using Lambda;
using StringTools;

#if macro
import haxe.macro.Context;
import haxe.macro.Expr;
#end

#if (js && debug)
private typedef PlatformName = String;
#elseif sys
private typedef PlatformName = String;
#else
private typedef PlatformName = Dynamic;
#end

/**
 * A hand-written delivery fixture for café orders in 東京.
 * @since 2.4
 * @see Parcel
 */
@:structInit
typedef Address = {
  final street:String;
  final city:String;
  ?postalCode:String;
}

typedef AuditRecord = {
  > Address,
  final createdAt:Date;
  ?note:String;
  function display():String;
}

enum DeliveryState {
  Queued;
  InTransit(vehicle:String, remainingKm:Float);
  Delivered(at:Date);
  Failed(?reason:String);
}

enum abstract Priority(Int) from Int to Int {
  var Low = 1;
  var Normal = 5;
  var Urgent = 10;

  @:op(A + B)
  public inline function add(other:Priority):Priority {
    return cast (this + other);
  }
}

interface Named {
  public var label(get, never):String;
  public function rename(next:String):Void;
}

interface Encodable {
  public function encode():String;
}

@:keep
class Payload implements Named implements Encodable {
  public var label(get, never):String;
  private var rawLabel:String;
  public final weight:Float;

  public function new(label:String, weight:Float) {
    this.rawLabel = label;
    this.weight = weight;
  }

  private function get_label():String {
    return rawLabel;
  }

  public function rename(next:String):Void {
    rawLabel = next.trim();
  }

  public function encode():String {
    return JsonCodec.stringify({label: rawLabel, weight: weight});
  }
}

@:generic
class Parcel<T:(Named & Encodable)> {
  public static inline final MAX_ATTEMPTS:Int = 0x0F;
  public static final ROUTE_MASK:Int = 0b1010_0110;
  public var status(default, set):DeliveryState;
  public var current(get, never):T;

  final item:T;
  var priority:Priority;
  var attempts:Int = 0;

  public function new(item:T, ?priority:Priority = Normal) {
    this.item = item;
    this.priority = priority;
    this.status = Queued;
  }

  inline function get_current():T return item;

  function set_status(next:DeliveryState):DeliveryState {
    return status = next;
  }

  /** @deprecated Prefer summary. */
  @:deprecated("Prefer summary")
  public function describe(prefix:String = "parcel"):String {
    var symbols = "café λ 東京 🚀 𝌆";
    return prefix + " " + item.label + ": " + symbols;
  }

  public function summary():String {
    return switch status {
      case Queued: "queued";
      case InTransit(vehicle, km) if (km > 0): vehicle + " has " + km + "km left";
      case InTransit(_, _): "arriving";
      case Delivered(at): "delivered " + at.toString();
      case Failed(null): "failed without a reason";
      case Failed(reason): "failed: " + reason;
    };
  }

  public function retry(backoff:(attempt:Int) -> Float):Float {
    attempts++;
    if (attempts >= MAX_ATTEMPTS) {
      throw "retry limit reached";
    }
    return backoff(attempts);
  }

  public function route(limit:Int):Array<Int> {
    var squares = [for (i in 0...limit) i * i];
    var filtered = squares.filter(value -> value % 2 == 0);
    for (index => value in filtered) {
      filtered[index] = value << 1;
    }
    return filtered;
  }

  public function inspect(value:Dynamic):String {
    if (value == null) return "nothing";
    return value is String ? cast(value, String).trim() : Std.string(value);
  }

  public function calculate(seed:Int):Int {
    var n = seed;
    n += 3;
    n *= 2;
    n = (n >>> 1) ^ ROUTE_MASK;
    n = (n & 0xFF) | 1;
    return n > 100 && priority >= Urgent ? n : -n;
  }

  public function parseCode(code:String):Null<String> {
    var pattern = ~/^(?:[A-Z][A-Za-z]+)-(\d{2,4})$/u;
    return pattern.match(code) ? pattern.matched(1) : null;
  }
}

class Stress {
  static function load(path:String):String {
    try {
      #if sys
      return File.getContent(path);
      #else
      return "unsupported";
      #end
    } catch (error:haxe.Exception) {
      trace(error.message);
      return "";
    }
  }

  static function buildIndex(values:Array<String>):StringMap<Int> {
    final result = new StringMap<Int>();
    for (index => value in values) {
      if (value.length == 0) continue;
      result.set(value, index);
    }
    return result;
  }

  static function transformations():Array<String->String> {
    var trim:String->String = text -> text.trim();
    var decorate = (text:String, ?mark:String = "!") -> '$text$mark';
    return [trim, text -> decorate(text.toUpperCase(), "…")];
  }

  public static function main():Void {
    var payload = new Payload("  launch-🚀  ", 12.5e1);
    payload.rename(payload.label);
    var parcel = new Parcel<Payload>(payload, Urgent + Low);
    parcel.status = InTransit("自転車", 3.5);

    final aliases = buildIndex(["café", "λ", "東京", "𝌆"]);
    final byId:IntMap<Payload> = [7 => payload];
    var message = parcel.summary() ?? "missing";
    var selected = byId[7]?.label;

    for (transform in transformations()) {
      trace(transform(message + " / " + selected + " / " + aliases.get("東京")));
    }

    do {
      parcel.calculate(aliases.get("café"));
    } while (false);

    switch [parcel.status, selected] {
      case [InTransit(_, distance), name] if (name != null):
        trace("moving " + distance + " for " + name);
      case [_, _]:
        trace("other");
    }

    untyped __js__("console.log({0})", payload.encode());
    trace(load("delivery.txt"));
  }
}

#if macro
class ParcelMacros {
  public static macro function announce(text:String):Expr {
    var position = Context.currentPos();
    return macro {
      var generated = $v{text};
      trace($e{macro generated}, $v{position});
    };
  }
}
#end
