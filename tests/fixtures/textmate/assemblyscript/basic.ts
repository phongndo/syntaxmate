export class Entry {
  constructor(public id: u64, public score: f64) {}
}

export function sum(values: StaticArray<i32>): i64 {
  let total: i64 = 0;
  for (let index = 0; index < values.length; index++) {
    total += unchecked(values[index]);
  }
  return total;
}

export function allocate(size: i32): usize {
  return heap.alloc(<usize>size);
}
