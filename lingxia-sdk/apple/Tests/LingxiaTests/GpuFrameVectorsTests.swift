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
            .appendingPathComponent("../../../../packages/lingxia-view/contracts/vectors/frame.json")
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
        XCTAssertTrue(nodes.contains { $0.testId == "plan" && $0.value == "B" })
        XCTAssertTrue(nodes.contains { $0.testId == "name" && $0.actions.contains { $0.kind == "paste" } })
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
            case "automationCall": XCTAssertGreaterThan(try JSONDecoder().decode(GpuFrameAutomationCall.self, from: value).call, 0)
            case "automationResult": XCTAssertTrue(try JSONDecoder().decode(GpuFrameAutomationResult.self, from: value).done)
            default: XCTFail("unknown reply \(reply)")
            }
        }
    }

    private func rebuild(_ e: GpuFrameInputEdit) -> GpuFrameInputEditMessage {
        switch e.op {
        case "selection": return .selection(anchor: e.anchor!, focus: e.focus!)
        case "composition": return .composition(start: e.start!, end: e.end!)
        case "replace": return .replace(text: e.text!, cursor: e.cursor!, composing: e.composing!)
        case "finish_composition": return .finishComposition
        case "undo": return .undo
        case "redo": return .redo
        default: return .deleteSurrounding(before: e.before!, after: e.after!, codePoints: e.codePoints!)
        }
    }

    private func rebuild(_ r: GpuFrameInputRequest) -> GpuFrameInputRequestMessage {
        switch r.op {
        case "drag_hit_test": return .dragHitTest(identity: r.identity!, generation: r.generation!, token: r.token!, point: r.point!, edge: r.edge!)
        case "hit_test": return .hitTest(identity: r.identity!, generation: r.generation!, point: r.point!, word: r.word!)
        case "focus": return .focus(identity: r.identity!, generation: r.generation!)
        case "query": return .query(identity: r.identity!, token: r.token!)
        case "select": return .select(identity: r.identity!, generation: r.generation!, index: r.index!)
        case "blur": return .blur(identity: r.identity!, token: r.token!)
        case "pick": return .pick(identity: r.identity!, token: r.token!, parts: r.parts)
        default: return .edit(identity: r.identity!, token: r.token!, edits: r.edits!.map(rebuild), submit: r.submit!)
        }
    }

    func testInputRequestMessagesEncodeEveryRequestTheCoreParses() throws {
        let root = try JSONSerialization.jsonObject(with: vectors()) as! [String: Any]
        let requests = root["inputRequests"] as! [[String: Any]]
        XCTAssertEqual(Set(requests.map { $0["op"] as! String }).count, 8)
        for vector in requests {
            let decoded = try JSONDecoder().decode(GpuFrameInputRequest.self, from: JSONSerialization.data(withJSONObject: vector))
            let encoded = try JSONSerialization.jsonObject(with: JSONEncoder().encode(rebuild(decoded))) as! NSDictionary
            XCTAssertTrue(encoded.isEqual(to: vector), "\(encoded) != \(vector)")
        }
    }

    func testInputRepliesCopyRootsAndTextMenuDecode() throws {
        let replies = try JSONDecoder().decode([GpuFrameInputReply].self, from: section("inputReplies"))
        XCTAssertTrue(replies.contains { $0.token != nil && $0.state?.focused == true })
        XCTAssertTrue(replies.contains { $0.selection?.count == 2 })
        _ = try JSONDecoder().decode(GpuFrameCopyReply.self, from: section("copySelection"))
        let roots = try JSONDecoder().decode(GpuFrameSemanticRoots.self, from: section("semanticRoots"))
        XCTAssertFalse(roots.roots.isEmpty)
        let menu = try JSONDecoder().decode(GpuFrameTextMenu.self, from: section("textMenu"))
        XCTAssertEqual(menu.target.kind, "editor")
        XCTAssertEqual(menu.actions.first, GpuFrameMenuAction.selectAll.rawValue)
    }
}
