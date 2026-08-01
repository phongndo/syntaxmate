# Compact Mojo sample with café and astral glyphs 🚀 𝄞.
struct Beacon:
    var label: String
    var power: Int

    fn __init__(out self, label: String, power: Int):
        self.label = label
        self.power = power

    fn announce(self):
        print(self.label, self.power)


fn doubled(value: Int) -> Int:
    return value * 2


fn main():
    let beacon = Beacon("café 🚀", 7)
    var total: Int = 0
    for step in range(1, 4):
        total += step
    if total < doubled(beacon.power):
        beacon.announce()
    else:
        print("quiet 𝄞")
