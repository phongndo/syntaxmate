#!/usr/bin/env dart
library fixtures.dart_basic;

import 'dart:math'
    show Random;

/// A tiny [Greeter] for café visitors and the rocket 🚀.
@Deprecated('Use Welcome instead')
class Greeter<T extends num> {
  Greeter(this.name, this.value);

  final String name;
  final T value;

  String message(int count) {
    final banner = "Hello, $name: ${count + value.toInt()}!";
    return '''$banner
Unicode stays intact: λ and 𝌆.
''';
  }
}

void main() {
  const attempts = 0x2A;
  print(Greeter<int>('Dart', attempts).message(Random().nextInt(4)));
}
