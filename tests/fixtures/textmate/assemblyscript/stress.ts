// AssemblyScript fixture: TypeScript syntax with WebAssembly scalar and memory types.
@unmanaged
class Vector4 {
  x: f32;
  y: f32;
  z: f32;
  w: f32;
}

export enum EntryState {
  Pending = 0,
  Active = 1,
  Archived = 2,
}

export class Entry {
  id: u64;
  score: f64;
  state: EntryState;

  constructor(id: u64, score: f64, state: EntryState = EntryState.Active) {
    this.id = id;
    this.score = score;
    this.state = state;
  }
}

@inline
function clampUnit(value: f64): f64 {
  return min<f64>(1.0, max<f64>(0.0, value));
}

export function sum(values: StaticArray<i32>): i64 {
  let total: i64 = 0;
  for (let index: i32 = 0; index < values.length; index++) {
    total += <i64>unchecked(values[index]);
  }
  return total;
}

export function transform0(value: f64): f64 {
  const scale: f64 = 1.0;
  const offset: f64 = 0.25;
  return clampUnit(value * scale + offset);
}

export function transform1(value: f64): f64 {
  const scale: f64 = 2.0;
  const offset: f64 = 1.25;
  return clampUnit(value * scale + offset);
}

export function transform2(value: f64): f64 {
  const scale: f64 = 3.0;
  const offset: f64 = 2.25;
  return clampUnit(value * scale + offset);
}

export function transform3(value: f64): f64 {
  const scale: f64 = 4.0;
  const offset: f64 = 3.25;
  return clampUnit(value * scale + offset);
}

export function transform4(value: f64): f64 {
  const scale: f64 = 5.0;
  const offset: f64 = 4.25;
  return clampUnit(value * scale + offset);
}

export function transform5(value: f64): f64 {
  const scale: f64 = 6.0;
  const offset: f64 = 5.25;
  return clampUnit(value * scale + offset);
}

export function transform6(value: f64): f64 {
  const scale: f64 = 7.0;
  const offset: f64 = 6.25;
  return clampUnit(value * scale + offset);
}

export function transform7(value: f64): f64 {
  const scale: f64 = 8.0;
  const offset: f64 = 7.25;
  return clampUnit(value * scale + offset);
}

export function transform8(value: f64): f64 {
  const scale: f64 = 9.0;
  const offset: f64 = 8.25;
  return clampUnit(value * scale + offset);
}

export function transform9(value: f64): f64 {
  const scale: f64 = 10.0;
  const offset: f64 = 9.25;
  return clampUnit(value * scale + offset);
}

export function transform10(value: f64): f64 {
  const scale: f64 = 11.0;
  const offset: f64 = 10.25;
  return clampUnit(value * scale + offset);
}

export function transform11(value: f64): f64 {
  const scale: f64 = 12.0;
  const offset: f64 = 11.25;
  return clampUnit(value * scale + offset);
}

export function transformBuffer(pointer: usize, count: i32): void {
  for (let index: i32 = 0; index < count; index++) {
    const address = pointer + <usize>index * sizeof<f64>();
    const value = load<f64>(address);
    store<f64>(address, transform11(value));
  }
}

export function dot(left: usize, right: usize, count: i32): f64 {
  let result: f64 = 0.0;
  for (let index: i32 = 0; index < count; index++) {
    result += load<f64>(left + <usize>index * 8) * load<f64>(right + <usize>index * 8);
  }
  return result;
}

export function classify(value: f64): EntryState {
  if (isNaN(value)) return EntryState.Pending;
  if (value > 0.5) return EntryState.Active;
  return EntryState.Archived;
}

export function rotate(value: v128): v128 {
  const shifted = i32x4.shl(value, 1);
  return v128.xor(value, shifted);
}

export function memoryPages(): i32 {
  return memory.size();
}
