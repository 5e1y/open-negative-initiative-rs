// `ReportSeal`'s steps on CryptoKit, for `report::tests::a_seal_crosses_to_cryptokit_and_back`:
// `swiftc -O tools/seal_cross.swift -o <tool>`, then run the test with `ONI_SEAL_CROSS=<tool>`.
import CryptoKit
import Foundation
// Usage: cross <private-key-b64> <blob-file> <text-to-seal>
let info = Data("ONI report log v1".utf8)
let prefix = "oni-log-v1:"
let args = CommandLine.arguments
let key = try! Curve25519.KeyAgreement.PrivateKey(rawRepresentation: Data(base64Encoded: args[1])!)
let blob = try! String(contentsOfFile: args[2], encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
let raw = Data(base64Encoded: String(blob.dropFirst(prefix.count)))!
let sender = try! Curve25519.KeyAgreement.PublicKey(rawRepresentation: raw.prefix(32))
let symmetric = try! key.sharedSecretFromKeyAgreement(with: sender).hkdfDerivedSymmetricKey(
    using: SHA256.self, salt: sender.rawRepresentation + key.publicKey.rawRepresentation,
    sharedInfo: info, outputByteCount: 32)
let packed = try! ChaChaPoly.open(ChaChaPoly.SealedBox(combined: raw.dropFirst(32)), using: symmetric)
let plain = try! (packed as NSData).decompressed(using: .zlib) as Data
print("OPENED:" + String(data: plain, encoding: .utf8)!)
// Sealed the Swift way to the same key, for the Rust side to open.
let recipient = key.publicKey
let compressed = try! (Data(args[3].utf8) as NSData).compressed(using: .zlib) as Data
let eph = Curve25519.KeyAgreement.PrivateKey()
let k = try! eph.sharedSecretFromKeyAgreement(with: recipient).hkdfDerivedSymmetricKey(
    using: SHA256.self, salt: eph.publicKey.rawRepresentation + recipient.rawRepresentation, sharedInfo: info, outputByteCount: 32)
let box = try! ChaChaPoly.seal(compressed, using: k)
print("SEALED:" + prefix + (eph.publicKey.rawRepresentation + box.combined).base64EncodedString())
