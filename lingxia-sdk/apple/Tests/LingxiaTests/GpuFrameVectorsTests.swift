import XCTest
@testable import lingxia

/// Replays the golden frame vectors the Rust host records through the generated decoder.
final class GpuFrameVectorsTests: XCTestCase {
    private struct Exchange: Decodable {
        let request: GpuFrameAutomationRequest
        let reply: String?
    }

    private func vectors() throws -> Data {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("../../../../packages/lingxia-ui/contracts/vectors/frame.json")
        return try Data(contentsOf: url.standardizedFileURL)
    }

    private func section(_ key: String) throws -> Data {
        let root = try JSONSerialization.jsonObject(with: vectors()) as! [String: Any]
        return try JSONSerialization.data(withJSONObject: root[key]!)
    }

    func testSnapshotDecodes() throws {
        let snapshot = try JSONDecoder().decode(GpuFrameFrameSnapshot.self, from: section("snapshot"))
        let byId = Dictionary(snapshot.nodes.filter { !$0.testId.isEmpty }.map { ($0.testId, $0) }, uniquingKeysWith: { a, _ in a })
        XCTAssertEqual(byId["name"]?.input?.keyboard, GpuFrameKeyboard.email.rawValue)
        XCTAssertEqual(byId["name"]?.bounds.count, 4)
        XCTAssertNotNil(byId["plan"]?.select)
        XCTAssertNotNil(byId["level"]?.range)
        XCTAssertNotNil(byId["chart"]?.plot)
        XCTAssertEqual(byId["title"]?.headingLevel, 1)
        XCTAssertNotNil(byId["inner"]?.scroll)
    }

    func testSemanticNodesDecode() throws {
        let nodes = try JSONDecoder().decode([GpuFrameSemanticNode].self, from: section("semanticNodes"))
        XCTAssertTrue(nodes.contains { $0.testId == "name" && $0.textField != nil })
        XCTAssertTrue(nodes.contains { $0.testId == "level" && $0.range != nil })
    }

    func testAutomationExchangesDecode() throws {
        let root = try JSONSerialization.jsonObject(with: vectors()) as! [String: Any]
        let exchanges = root["automation"] as! [[String: Any]]
        XCTAssertFalse(exchanges.isEmpty)
        for exchange in exchanges {
            let data = try JSONSerialization.data(withJSONObject: exchange)
            let decoded = try JSONDecoder().decode(Exchange.self, from: data)
            guard let reply = decoded.reply else {
                XCTAssertEqual(decoded.request.action?.kind, "fill")
                continue
            }
            let value = try JSONSerialization.data(withJSONObject: exchange["value"]!)
            switch reply {
            case "automationElement": _ = try JSONDecoder().decode(GpuFrameAutomationElement.self, from: value)
            case "automationQueryAll": XCTAssertGreaterThan(try JSONDecoder().decode(GpuFrameAutomationQueryAll.self, from: value).count, 0)
            case "automationTree": XCTAssertFalse(try JSONDecoder().decode(GpuFrameAutomationTree.self, from: value).nodes.isEmpty)
            case "automationFrame": _ = try JSONDecoder().decode(GpuFrameAutomationFrame.self, from: value)
            default: XCTFail("unknown reply \(reply)")
            }
        }
    }
}
