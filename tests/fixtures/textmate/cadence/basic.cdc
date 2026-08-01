// Cadence basics: café, λ, 東京, 🚀, 𝌆
import FungibleToken from 0x01

access(all) contract Hello {
    access(all) event Greeted(name: String, count: Int)
    access(all) let prefix: String

    init() {
        self.prefix = "Bonjour, café \u{2615}"
    }

    access(all) view fun greet(
        _ name: String,
        times count: Int
    ): String {
        let message = "\(self.prefix), \(name)! 🚀"
        if count > 0 && name != "" {
            emit Greeted(name: name, count: count)
            return message
        } else {
            return "東京 λ 𝌆"
        }
    }
}
