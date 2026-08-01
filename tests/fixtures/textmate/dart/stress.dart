#!/usr/bin/env dart
library fixtures.dart_stress;

import 'dart:async'
    show Future,
        Stream;
import 'dart:math' as math;
export 'src/public_api.dart'
    hide InternalToken;
part 'stress.g.dart';
/// Grammar fixture for [Envelope], `decode`, café text, and 🚀 payloads.
///
/// ```dart
/// final sample = Envelope<String>('λ');
/// print(sample.value);
/// ```
@Deprecated('Kept for oracle coverage')
typedef Decoder<T extends Object?> = T Function(String source);
/**
 * An old-style documentation block mentions [Result] and `fold`.
 *
 * ```dart
 * final answer = Success<int>(42);
 * print(answer);
 * ```
 *     indented source is also recognized
 */
abstract interface class Codec<T extends Object?> {
  const Codec();
  T decode(String source);
  String encode(covariant T value);
}
base mixin DiagnosticMixin {
  String get label => runtimeType.toString();
}
sealed class Result<T extends Object?> with DiagnosticMixin {
  const Result();
  R fold<R extends Object?>(R Function(T) ok, R Function(Object) bad);
}
final class Success<T extends Object?> extends Result<T> {
  const Success(this.value);
  final T value;

  @override
  R fold<R extends Object?>(R Function(T) ok, R Function(Object) bad) {
    return ok(value);
  }
}
final class Failure<T extends Object?> extends Result<T> {
  const Failure(this.error);
  final Object error;

  @override
  R fold<R extends Object?>(R Function(T) ok, R Function(Object) bad) =>
      bad(error);
}
enum Flavor {
  vanilla,
  chocolate,
  caféAuLait;

  bool get isSweet => this != caféAuLait;
}

extension type UserId(int value) implements int {
  bool get isPositive => value > 0;
}

extension StringTools on String {
  String get shouted => '${toUpperCase()}!';
  bool get hasUnicode => contains('λ') || contains('🚀');
}

class Envelope<T extends Object?> implements Codec<T> {
  Envelope(this.value, {this.tag = 'plain'});
  factory Envelope.empty(T fallback) => Envelope<T>(fallback);
  external factory Envelope.external(T value);

  static const int mask = 0xFF_A0;
  static final double ratio = .625e+2;
  late final String memo;
  final T value;
  final String tag;

  @override
  T decode(String source) => value;

  @override
  String encode(covariant T item) => '$tag:${item.toString()}';

  T get current => this.value;
  set description(String next) => memo = next;

  Envelope<T> operator +(Envelope<T> other) => Envelope<T>(other.value);
}

/* A multiline comment holds state across physical lines.
   Nested comments are supported: /* inner café λ */
   The outer level closes here, before code resumes. */
/**/

String strings(String name, int count) {
  final escaped = "quote: \"; slash: \\; newline: \n; $name";
  final single = 'count=${count + 1}, hex=${Envelope.mask}';
  final rawDouble = r"C:\temp\new\$name";
  final rawSingle = r'raw \n ${notInterpolated}';
  final multiline = """First line: café.
Second line interpolates $name and ${count * 2}.
Astral symbols remain UTF-16 pairs: 🚀 and 𝌆.
""";
  final multilineSingle = '''alpha
beta ${name.toUpperCase()}
gamma λ
''';
  final rawTripleDouble = r"""raw $name \n "quotes"
second raw line with 🚀
""";
  final rawTripleSingle = r'''raw ${count} and 'quotes'
second raw line with 𝌆
''';
  return [escaped, single, rawDouble, rawSingle, multiline,
          multilineSingle, rawTripleDouble, rawTripleSingle].join('|');
}

num arithmetic(int left, int right, bool enabled) {
  var value = left + right - 2 * 3 / 4;
  value ~/= 2;
  value %= 7;
  value += 3;
  value -= 1;
  final bits = ((left << 2) | right) ^ (left & right);
  final shifted = (bits >> 1) + (bits >>> 2);
  var flags = ~shifted;
  flags &= 0xFF;
  flags |= 0x10;
  flags ^= 0x01;
  flags <<= 1;
  flags >>= 1;
  flags >>>= 1;
  value++;
  value--;
  return enabled && left != right || !enabled ? value + flags : -value;
}

String classify(Object? input) {
  assert(input == null || input is Object);
  if (input is! String) {
    return input == null ? 'null' : input.toString();
  } else if (input.isEmpty) {
    return 'empty';
  }
  switch (input.length) {
    case 1:
      return 'one';
    case int size when size > 8:
      return 'long:$size';
    default:
      return input as String;
  }
}

Iterable<int> countdown(int start) sync* {
  var current = start;
  do {
    if (current == 2) {
      current--;
      continue;
    }
    yield current;
  } while (current-- > 0);
}

Stream<int> timedValues() async* {
  await Future<void>.delayed(const Duration(milliseconds: 1));
  yield* Stream<int>.fromIterable(countdown(4));
}

Future<Result<int>> guardedDecode(String source) async {
  try {
    if (source.isEmpty) throw const FormatException('empty');
    return Success<int>(int.parse(source));
  } on FormatException catch (error) {
    return Failure<int>(error);
  } catch (error) {
    rethrow;
  } finally {
    print('decode finished');
  }
}

void loops(List<String> words) {
  for (final word in words) {
    if (word == 'stop') break;
    print(word);
  }
  for (var index = 0; index < words.length; index++) {
    words[index] = words[index].shouted;
  }
  while (words.isEmpty) {
    return;
  }
}

external int platformHash(String value);
int legacyHash(String value) native 'legacyHash';

void main() async {
  final codec = new Envelope<String>('café 🚀', tag: 'λ');
  codec.description = strings('Dart', 3);
  await for (final value in timedValues()) {
    print('${codec.encode(codec.current)} => $value');
  }
  final result = await guardedDecode('42');
  print(result.fold<String>((value) => 'ok:$value', (error) => 'bad:$error'));
  loops(<String>['alpha', 'beta', 'stop']);
  print(arithmetic(12, 5, true));
}
