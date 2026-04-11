/// A type-safe representation of any JSON value.
///
/// Used as the currency type for unstructured data flowing between
/// the SDK and host apps.
public enum LxAppJSONValue: Codable, Sendable, Hashable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([LxAppJSONValue])
    case object([String: LxAppJSONValue])

    // MARK: - Codable

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([LxAppJSONValue].self) {
            self = .array(value)
        } else if let value = try? container.decode([String: LxAppJSONValue].self) {
            self = .object(value)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "LxAppJSONValue cannot decode value"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null:
            try container.encodeNil()
        case .bool(let v):
            try container.encode(v)
        case .number(let v):
            try container.encode(v)
        case .string(let v):
            try container.encode(v)
        case .array(let v):
            try container.encode(v)
        case .object(let v):
            try container.encode(v)
        }
    }

    // MARK: - Convenience

    /// Access a nested value by key (returns `nil` if not an object or key missing).
    public subscript(key: String) -> LxAppJSONValue? {
        guard case .object(let dict) = self else { return nil }
        return dict[key]
    }

    /// Access an element by index (returns `nil` if not an array or index out of bounds).
    public subscript(index: Int) -> LxAppJSONValue? {
        guard case .array(let arr) = self, arr.indices.contains(index) else { return nil }
        return arr[index]
    }
}
