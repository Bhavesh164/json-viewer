import Foundation
import JSONViewerCore

var totalTests = 0
var passedTests = 0
var failedTests = 0

/// Search runs off the main thread, so the assertions have to let it land.
///
/// The suite is async, but `searchSubmit` returns before its detached task finishes
/// and commits results. Polling here is what makes these assertions meaningful rather
/// than a race against the scheduler.
@MainActor
func waitFor(_ description: String, timeout: TimeInterval = 5.0, _ condition: () -> Bool) async {
    let deadline = Date().addingTimeInterval(timeout)
    while !condition() {
        guard Date() < deadline else {
            assertTest(false, "Timed out waiting for: \(description)")
            return
        }
        // Yield so the search task can run, then give it a real slice of time in case
        // it is still doing work off the main thread.
        await Task.yield()
        try? await Task.sleep(nanoseconds: 5_000_000)
    }
}

/// Wait for the next rebuild to be applied.
///
/// `isParsing` is still false while a rebuild sits in the debounce, so waiting on it
/// alone returns before anything has happened.
@MainActor
func waitForRebuild(_ model: JSONDocumentModel, from baseline: Int, timeout: TimeInterval = 20.0) async {
    await waitFor("a rebuild to be applied", timeout: timeout) { model.rebuildCount > baseline }
}

/// Stand-in for the native editor in the text-sync tests: it owns the text and the
/// model has to ask for it.
@MainActor
final class FakeTextSource: JSONDocumentTextSource {
    var text: String
    var isActiveEditor: Bool
    init(text: String, isActiveEditor: Bool = true) {
        self.text = text
        self.isActiveEditor = isActiveEditor
    }
    var currentText: String { text }
}

func assertTest(_ condition: Bool, _ name: String, file: String = #file, line: Int = #line) {
    totalTests += 1
    if condition {
        passedTests += 1
        print("  ✅ PASS: \(name)")
    } else {
        failedTests += 1
        print("  ❌ FAIL: \(name) at \(file):\(line)")
    }
}

@MainActor
func runSuite() async {
    print("\n🚀 Running JSONViewer Test Suite...")


// 1. Test Primitives
do {
    let json = """
    {
        "stringVal": "hello world",
        "intVal": 42,
        "floatVal": 3.14159,
        "boolTrue": true,
        "boolFalse": false,
        "nullVal": null
    }
    """
    let val = try JSONParser.parse(json)
    if case .object(let pairs) = val {
        assertTest(pairs.count == 6, "Object should contain 6 pairs")
        assertTest(pairs[0].key == "stringVal" && pairs[0].value == .string("hello world"), "String parsed correctly")
        assertTest(pairs[3].key == "boolTrue" && pairs[3].value == .bool(true), "Bool true parsed correctly")
        assertTest(pairs[5].key == "nullVal" && pairs[5].value == .null, "Null parsed correctly")
    } else {
        assertTest(false, "Expected object for primitives")
    }
} catch {
    assertTest(false, "Failed to parse primitives: \(error)")
}

// 2. Test Preserving Key Order
do {
    let json = """
    {
        "zebra": 1,
        "apple": 2,
        "mango": 3,
        "banana": 4
    }
    """
    let val = try JSONParser.parse(json)
    if case .object(let pairs) = val {
        let keys = pairs.map { $0.key }
        assertTest(keys == ["zebra", "apple", "mango", "banana"], "Preserves exact key order")
    } else {
        assertTest(false, "Expected object for key order")
    }
} catch {
    assertTest(false, "Failed to test key order: \(error)")
}

// 3. Test 2-Space Indented Formatting (Matching jsonviewer.stack.hu)
do {
    let json = "{\"user\":{\"id\":1,\"tags\":[\"admin\",\"staff\"]}}"
    let val = try JSONParser.parse(json)
    let formatted = val.format(indentSpaces: 2)
    let expected = """
    {
      "user": {
        "id": 1,
        "tags": [
          "admin",
          "staff"
        ]
      }
    }
    """
    assertTest(formatted == expected, "2-Space Pretty Print Formatting matches jsonviewer.stack.hu")
} catch {
    assertTest(false, "Failed to test formatting: \(error)")
}

// 4. Test Minify (Whitespace removal outside strings)
do {
    let json = """
    {
      "user": {
        "id": 100,
        "name": "John Doe",
        "active": true
      }
    }
    """
    let val = try JSONParser.parse(json)
    let minified = val.minify()
    let expected = "{\"user\":{\"id\":100,\"name\":\"John Doe\",\"active\":true}}"
    assertTest(minified == expected, "Minification removes extra whitespace")
} catch {
    assertTest(false, "Failed to test minify: \(error)")
}

// 5. Test Tree Building and JSONPath Generation
do {
    let json = """
    {
      "store": {
        "book": [
          { "title": "Swift Programming", "price": 49.99 }
        ]
      }
    }
    """
    let val = try JSONParser.parse(json)
    let root = JSONNode.buildTree(from: val, rootKey: "JSON")
    
    assertTest(root.key == "JSON", "Root key is JSON")
    assertTest(root.children?.count == 1, "Root has 1 child")
    
    if let store = root.children?.first {
        assertTest(store.key == "store", "Store key matches")
        assertTest(store.path == "$.store", "Store path is $.store")
        
        if let book = store.children?.first {
            assertTest(book.key == "book", "Book key matches")
            assertTest(book.path == "$.store.book", "Book path is $.store.book")
            
            if let firstBook = book.children?.first {
                assertTest(firstBook.key == "0", "Array index 0 matches")
                assertTest(firstBook.path == "$.store.book[0]", "First book path is $.store.book[0]")
                
                if let titleNode = firstBook.children?.first {
                    assertTest(titleNode.key == "title", "Title key matches")
                    assertTest(titleNode.path == "$.store.book[0].title", "Title path is $.store.book[0].title")
                    assertTest(titleNode.displayText == "title : \"Swift Programming\"", "Display text format matches")
                }
            }
        }
    }
} catch {
    assertTest(false, "Failed to test tree building: \(error)")
}

// Repeated object keys are kept by the parser, but the tree shows one row per key:
// the last value, as JSON.parse resolves it, at the position of the first.
do {
    let val = try JSONParser.parse("{\"count\":3,\"count\":23423,\"count\":23423}")
    assertTest(val == .object([
        JSONProperty(key: "count", value: .number(3, raw: "3")),
        JSONProperty(key: "count", value: .number(23423, raw: "23423")),
        JSONProperty(key: "count", value: .number(23423, raw: "23423"))
    ]), "Parser keeps every repeated object key")
    let root = JSONNode.buildTree(from: val, rootKey: "JSON")
    let countNodes = root.children ?? []
    assertTest(countNodes.count == 1, "A repeated object key collapses to one tree row")
    assertTest(countNodes.first?.valueString == "23423", "A repeated object key collapses to the last value")
    assertTest(countNodes.first?.path == "$.count", "The surviving duplicate keeps the first occurrence's path")
} catch {
    assertTest(false, "Failed to test duplicate object keys: \(error)")
}

// Repeated keys must not disturb their siblings, and must collapse per object
// rather than per document.
do {
    let val = try JSONParser.parse("{\"a\":1,\"k\":1,\"b\":2,\"k\":9}")
    let root = JSONNode.buildTree(from: val, rootKey: "JSON")
    assertTest(root.children?.map { $0.key } == ["a", "k", "b"], "Duplicate keys keep their position and do not disturb siblings")
    assertTest(root.children?.map { $0.valueString } == ["1", "9", "2"], "Duplicate keys show the last value in place")

    let arrayVal = try JSONParser.parse("[{\"k\":1},{\"k\":2}]")
    let arrayRoot = JSONNode.buildTree(from: arrayVal, rootKey: "JSON")
    assertTest(arrayRoot.children?.count == 2, "Duplicate keys in an array element do not collapse across elements")
    assertTest(arrayRoot.children?.allSatisfy { $0.children?.count == 1 } == true, "Each array element collapses its own duplicate keys")
} catch {
    assertTest(false, "Failed to test duplicate key positions: \(error)")
}

// 6. Test Property Grid Mapping
do {
    let json = """
    {
      "name": "Alice",
      "age": 30,
      "city": "Cupertino"
    }
    """
    let val = try JSONParser.parse(json)
    let root = JSONNode.buildTree(from: val, rootKey: "JSON")
    let props = root.propertiesForGrid()
    assertTest(props.count == 3, "Property grid contains 3 properties")
    assertTest(props[0].name == "name" && props[0].value == "Alice", "First property matches")
    assertTest(props[1].name == "age" && props[1].value == "30", "Second property matches")
    
    // Leaf node displays its parent container's properties
    if let leaf = root.children?.first {
        let leafProps = leaf.propertiesForGrid()
        assertTest(leafProps.count == 3, "Leaf node property inspection displays parent container's properties")
    }
} catch {
    assertTest(false, "Failed to test property grid: \(error)")
}

// 7. Test Search Matching
do {
    let json = """
    {
      "contacts": [
        { "name": "John Doe", "email": "john@example.com" },
        { "name": "Jane Smith", "email": "jane@example.com" }
      ]
    }
    """
    let val = try JSONParser.parse(json)
    let root = JSONNode.buildTree(from: val, rootKey: "JSON")
    let matches = root.searchMatches(query: "Jane")
    assertTest(!matches.isEmpty, "Search finds matches for 'Jane'")
    assertTest(matches.contains { $0.displayText.contains("Jane") }, "Match contains 'Jane'")
} catch {
    assertTest(false, "Failed to test search: \(error)")
}

// 7b. Test Search Navigation with Enter and Shift+Enter
do {
    let json = """
    {
      "users": [
        { "name": "Alice", "role": "admin", "domain": "test.com" },
        { "name": "Bob", "role": "user", "domain": "test.com" },
        { "name": "Charlie", "role": "admin", "domain": "test.com" }
      ]
    }
    """
    let model = JSONDocumentModel()
    model.rawText = json
    _ = model.parseAndBuildTree(silent: true)
    
    // Initial search for "test.com" (3 matches) via Enter
    model.searchQuery = "test.com"
    model.searchSubmit(reverse: false)
    await waitFor("the 'test.com' search to finish") { model.searchStatus != "Searching\u{2026}" }
    assertTest(model.searchResults.count == 3, "Search finds 3 matches for 'test.com'")
    assertTest(model.currentSearchIndex == 0, "First match selected at index 0")
    assertTest(model.searchStatus == "1 of 3 matches", "Status displays '1 of 3 matches'")
    
    // Press Enter -> should advance to match 2
    model.searchSubmit(reverse: false)
    assertTest(model.currentSearchIndex == 1, "Second Enter advances to index 1")
    assertTest(model.searchStatus == "2 of 3 matches", "Status displays '2 of 3 matches'")
    
    // Press Enter -> should advance to match 3
    model.searchSubmit(reverse: false)
    assertTest(model.currentSearchIndex == 2, "Third Enter advances to index 2")
    assertTest(model.searchStatus == "3 of 3 matches", "Status displays '3 of 3 matches'")
    
    // Press Enter on last match -> should wrap back to match 1
    model.searchSubmit(reverse: false)
    assertTest(model.currentSearchIndex == 0, "Enter on last match wraps to index 0")
    assertTest(model.searchStatus == "1 of 3 matches", "Status displays '1 of 3 matches'")
    
    // Press Shift+Enter -> should reverse back to match 3
    model.searchSubmit(reverse: true)
    assertTest(model.currentSearchIndex == 2, "Shift+Enter wraps back to index 2")
    assertTest(model.searchStatus == "3 of 3 matches", "Status displays '3 of 3 matches'")
    
    // Press Shift+Enter -> should reverse back to match 2
    model.searchSubmit(reverse: true)
    assertTest(model.currentSearchIndex == 1, "Shift+Enter reverses to index 1")
    assertTest(model.searchStatus == "2 of 3 matches", "Status displays '2 of 3 matches'")
    
    // Query change: type "admin" (2 matches) and press Enter
    model.searchQuery = "admin"
    model.searchSubmit(reverse: false)
    await waitFor("the 'admin' search to finish") { model.searchStatus != "Searching\u{2026}" }
    assertTest(model.searchResults.count == 2, "New search 'admin' finds 2 matches")
    assertTest(model.currentSearchIndex == 0, "New search resets to index 0")
    assertTest(model.searchStatus == "1 of 2 matches", "Status displays '1 of 2 matches'")
    
    // Query change: non-existent phrase
    model.searchQuery = "nonexistentxyz"
    model.searchSubmit(reverse: false)
    await waitFor("the 'nonexistentxyz' search to finish") { model.searchStatus != "Searching\u{2026}" }
    assertTest(model.searchResults.isEmpty, "Non-existent search produces 0 matches")
    assertTest(model.searchStatus == "Phrase not found!", "Status displays 'Phrase not found!'")
    
    // Clear search
    model.clearSearch()
    assertTest(model.searchQuery.isEmpty && model.searchResults.isEmpty && model.searchStatus.isEmpty, "clearSearch resets all search state")
}

// 7c. Test Properties Visibility Toggle
do {
    let model = JSONDocumentModel()
    assertTest(model.isPropertiesVisible == true, "Properties visible by default")
    model.toggleProperties()
    assertTest(model.isPropertiesVisible == false, "Properties hidden after toggle")
    model.toggleProperties()
    assertTest(model.isPropertiesVisible == true, "Properties shown again after second toggle")
}

// 8. Test Error Reporting on Malformed JSON
do {
    let malformed = """
    {
      "validKey": "validVal",
      "badKey": 
    }
    """
    do {
        _ = try JSONParser.parse(malformed)
        assertTest(false, "Should throw error on malformed JSON")
    } catch let err as JSONParseError {
        assertTest(err.line == 4, "Parse error correctly identifies line 4")
    } catch {
        assertTest(false, "Expected JSONParseError, got \(error)")
    }
}
// 9. Benchmark 5MB JSON
if FileManager.default.fileExists(atPath: "sample_5mb.json") {
    do {
        let t0 = Date()
        let str = try String(contentsOfFile: "sample_5mb.json")
        let readTime = Date().timeIntervalSince(t0)
        
        let t1 = Date()
        let parsed = try JSONParser.parse(str)
        let parseTime = Date().timeIntervalSince(t1)
        
        let t2 = Date()
        let root = JSONNode.buildTree(from: parsed)
        let treeTime = Date().timeIntervalSince(t2)
        
        print("⚡️ [PERF 5MB] Read: \(String(format: "%.3f", readTime))s, Parse: \(String(format: "%.3f", parseTime))s, BuildTree: \(String(format: "%.3f", treeTime))s, Total: \(String(format: "%.3f", readTime + parseTime + treeTime))s")
        assertTest(root.children?.count == 25000, "5MB tree root has 25,000 children")
        
        // 10. Test JSONDocumentModel with 5MB JSON
        let model = JSONDocumentModel()
        model.rawText = str
        let success = model.parseAndBuildTree(silent: true)
        assertTest(success, "5MB JSON parsed successfully into document model")
        assertTest(model.expandedNodeIds.count == 1, "Initial expansion contains ONLY root node (1 node)")
        assertTest(model.visibleTreeRows.count == 25001, "Visible rows initially has 25,001 rows (root + 25k direct children)")
        assertTest(model.visibleTreeRows[1].isExpanded == false, "First child node (0) is initially collapsed")
        assertTest(model.visibleTreeRows[1].isContainer == true, "First child node (0) is a container")
        
        // Test expanding child 0
        let firstChildId = model.visibleTreeRows[1].node.id
        model.toggleExpand(nodeId: firstChildId)
        assertTest(model.expandedNodeIds.contains(firstChildId), "Child 0 is now expanded")
        assertTest(model.visibleTreeRows.count > 25001, "Visible rows increased after expanding child 0")
        
        // Test collapseAll
        model.collapseAll()
        assertTest(model.expandedNodeIds.count == 1, "After collapseAll, only root remains expanded")
        assertTest(model.visibleTreeRows.count == 25001, "Visible rows back to 25,001")
        
        // Test property grid with root selected
        assertTest(model.selectedNodeProperties.count == 25000, "Property grid reflects all 25,000 direct children")
    } catch {
        print("❌ 5MB Benchmark failed: \(error)")
    }
}

// 11. Test Font Scaling & Zoom controls
do {
    let model = JSONDocumentModel()
    assertTest(model.fontSize == 12.0, "Initial font size is 12.0")
    model.zoomIn()
    assertTest(model.fontSize == 13.0, "Zoom in increases font size to 13.0")
    model.zoomOut()
    assertTest(model.fontSize == 12.0, "Zoom out decreases font size back to 12.0")
    model.resetZoom()
    assertTest(model.fontSize == 12.0, "Reset zoom sets font size to 12.0")
    for _ in 0..<30 { model.zoomOut() }
    assertTest(model.fontSize == 9.0, "Font size lower limit bounded at 9.0")
    for _ in 0..<30 { model.zoomIn() }
    assertTest(model.fontSize == 24.0, "Font size upper limit bounded at 24.0")
    model.resetZoom()
    assertTest(model.fontSize == 12.0, "Reset zoom returns to 12.0 from max")
}

// 12. Test exact dataset from user screenshot: edge_5mb.json (15,840 items)
if FileManager.default.fileExists(atPath: "edge_5mb.json") {
    do {
        let str = try String(contentsOfFile: "edge_5mb.json")
        let model = JSONDocumentModel()
        model.rawText = str
        let success = model.parseAndBuildTree(silent: true)
        assertTest(success, "edge_5mb.json parsed successfully")
        
        guard let root = model.rootNode else {
            assertTest(false, "Root node should exist for edge_5mb.json")
            throw NSError(domain: "test", code: 1)
        }
        
        assertTest(root.children?.count == 15840, "edge_5mb.json has exactly 15,840 items matching screenshot")
        assertTest(model.visibleTreeRows.count == 15841, "Visible rows initially has 15,841 rows (root + 15,840 items)")
        
        // Inspect item 0 ("Adeel Solangi")
        let item0Row = model.visibleTreeRows[1]
        assertTest(item0Row.node.key == "0", "Item 0 key is '0'")
        assertTest(item0Row.node.typeBadgeText == "{5}", "Item 0 has '{5}' badge")
        assertTest(item0Row.isContainer == true, "Item 0 is a container object")
        
        // Expand item 0
        model.toggleExpand(nodeId: item0Row.node.id)
        assertTest(model.visibleTreeRows.count == 15846, "Visible rows expanded by 5 to 15,846")
        
        // Check expanded children
        let nameRow = model.visibleTreeRows[2]
        assertTest(nameRow.node.key == "name", "First property key is 'name'")
        if case .string(let val) = nameRow.node.value {
            assertTest(val == "Adeel Solangi", "Name value matches screenshot: 'Adeel Solangi'")
        } else {
            assertTest(false, "Name should be a string")
        }
        
        let langRow = model.visibleTreeRows[3]
        assertTest(langRow.node.key == "language", "Second property key is 'language'")
        if case .string(let val) = langRow.node.value {
            assertTest(val == "Sindhi", "Language value matches screenshot: 'Sindhi'")
        } else {
            assertTest(false, "Language should be a string")
        }
        
        let idRow = model.visibleTreeRows[4]
        assertTest(idRow.node.key == "id", "Third property key is 'id'")
        if case .string(let val) = idRow.node.value {
            assertTest(val == "V59OF92YF627HFY0", "ID value matches screenshot: 'V59OF92YF627HFY0'")
        } else {
            assertTest(false, "ID should be a string")
        }
        
        // Select item 0 and inspect property grid
        model.selectedNode = item0Row.node
        assertTest(model.selectedNodeProperties.count == 5, "Item 0 has 5 properties in grid")
        let propNames = model.selectedNodeProperties.map { $0.name }
        assertTest(propNames.contains("name") && propNames.contains("language") && propNames.contains("id") && propNames.contains("bio") && propNames.contains("version"), "Property grid contains all 5 fields")
        
        // 13. Test clicking element 509 in properties grid
        model.selectedNode = root
        assertTest(model.selectedNodeProperties.count == 15840, "Root has 15,840 properties in grid")
        
        // Find row 509 in property grid
        let row509 = model.selectedNodeProperties[509]
        assertTest(row509.name == "509", "Row 509 has name '509'")
        assertTest(!row509.nodeId.isEmpty, "Row 509 has valid nodeId")
        
        // Click / navigate to element 509
        model.navigateToProperty(row509)
        assertTest(model.selectedNode?.key == "509", "Navigating to property 509 selects node 509 in tree")
        assertTest(model.selectedNode?.path == "$[509]", "Node 509 path is $[509]")
        assertTest(model.expandedNodeIds.contains(root.id), "Ancestors of 509 are expanded")
        assertTest(model.selectedNodeProperties.count == 5, "Property grid now shows the 5 properties of element 509")
        
        // Test navigate to parent
        model.navigateToParent()
        assertTest(model.selectedNode?.key == "JSON", "Navigate to parent returns to Root JSON")
        
        // Test clicking leaf property in property grid (e.g. property 'name' in item 0)
        model.selectedNode = item0Row.node
        if let nameProp = model.selectedNodeProperties.first(where: { $0.name == "name" }) {
            model.navigateToProperty(nameProp)
            assertTest(model.selectedNode?.key == "name", "Clicking property 'name' navigates to leaf node 'name'")
            assertTest(model.selectedNode?.path == "$[0].name", "Leaf node 'name' path is $[0].name")
        }
        
        // 14. Test big text expansion on leaf node (e.g. 'bio')
        model.selectedNode = item0Row.node
        if let bioProp = model.selectedNodeProperties.first(where: { $0.name == "bio" }) {
            model.navigateToProperty(bioProp)
            assertTest(model.selectedNode?.key == "bio", "Selected bio node")
            if let bioNode = model.selectedNode {
                assertTest(bioNode.valueString.count > 40, "Bio has long text (> 40 chars)")
                assertTest(!model.expandedLeafNodeIds.contains(bioNode.id), "Initially leaf is collapsed")
                model.toggleExpandLeaf(nodeId: bioNode.id)
                assertTest(model.expandedLeafNodeIds.contains(bioNode.id), "Leaf node bio is expanded")
                model.toggleExpandLeaf(nodeId: bioNode.id)
                assertTest(!model.expandedLeafNodeIds.contains(bioNode.id), "Leaf node bio collapsed back")
            }
        }
    } catch {
        print("❌ edge_5mb.json test failed: \(error)")
    }
}

// 15. Test Stringify with forward slash escaping (\/) and quote escaping (\")
do {
    let json = """
    {
      "api": "/v1/users",
      "website": "https://example.com/test",
      "message": "hello \\"world\\""
    }
    """
    let val = try JSONParser.parse(json)
    
    // Test with slash escaping enabled (default for stringify)
    let stringifiedWithSlashes = val.stringify(escapeSlashes: true)
    assertTest(stringifiedWithSlashes.hasPrefix("\"") && stringifiedWithSlashes.hasSuffix("\""), "Stringified JSON starts and ends with quotes")
    assertTest(stringifiedWithSlashes.contains("\\/v1\\/users"), "Forward slashes are escaped as \\/ when escapeSlashes: true")
    assertTest(stringifiedWithSlashes.contains("\\\"api\\\""), "Quotes around keys are escaped as \\\"")
    assertTest(stringifiedWithSlashes.contains("\\\\\\\"world\\\\\\\""), "Quotes inside string values are escaped as \\\\\\\"")
    
    // Test with slash escaping disabled
    let stringifiedWithoutSlashes = val.stringify(escapeSlashes: false)
    assertTest(stringifiedWithoutSlashes.contains("/v1/users"), "Forward slashes remain unescaped when escapeSlashes: false")
} catch {
    assertTest(false, "Failed to test stringify: \(error)")
}

// 16. Test Unescaping stringified JSON containing escaped slashes and quotes
do {
    let stringifiedInput = "\"{\\\"api\\\":\\\"\\\\/v1\\\\/users\\\",\\\"count\\\":10,\\\"active\\\":true}\""
    let unescaped = JSONValue.unescapeStringifiedJSON(stringifiedInput)
    assertTest(!unescaped.hasPrefix("\"{\\\""), "Unescape strips string literal outer quotes and unescapes quotes")
    
    let parsedUnescaped = try? JSONParser.parse(unescaped)
    assertTest(parsedUnescaped != nil, "Unescaped result parses as valid JSON")
    if case .object(let pairs) = parsedUnescaped {
        assertTest(pairs.count == 3, "Unescaped object has 3 properties")
        assertTest(pairs[0].key == "api", "First property key is 'api'")
    }
}

// 17. Test Python Object Serialization (toPythonObject)
do {
    let json = """
    {
      "name": "Alice",
      "is_admin": true,
      "is_guest": false,
      "address": null,
      "score": 100,
      "tags": ["admin", "staff"]
    }
    """
    let val = try JSONParser.parse(json)
    let py = val.toPythonObject(indentSpaces: 4)
    
    assertTest(py.contains("'is_admin': True"), "JSON true serialized to Python True")
    assertTest(py.contains("'is_guest': False"), "JSON false serialized to Python False")
    assertTest(py.contains("'address': None"), "JSON null serialized to Python None")
    assertTest(py.contains("'name': 'Alice'"), "JSON string serialized to Python 'Alice'")
    assertTest(py.contains("[\n        'admin',\n        'staff'\n    ]"), "JSON array serialized to Python list")
    assertTest(py.hasPrefix("{\n") && py.hasSuffix("}"), "Python dictionary properly structured with brackets")
} catch {
    assertTest(false, "Failed to test Python object serialization: \(error)")
}

// 18. Test Auto-unwrapping stringified JSON in JSONDocumentModel
do {
    let model = JSONDocumentModel()
    model.settings.autoUnwrapStringified = true
    
    // Paste stringified JSON into model
    model.rawText = "\"{\\\"service\\\":\\\"auth\\\",\\\"endpoints\\\":[\\\"\\\\/login\\\",\\\"\\\\/logout\\\"]}\""
    let success = model.parseAndBuildTree(silent: true)
    assertTest(success, "Stringified JSON parsed and unwrapped successfully")
    assertTest(model.rootNode?.children?.count == 2, "Root node directly contains 2 children from unwrapped object")
    assertTest(model.rootNode?.children?[0].key == "service", "First unwrapped child is 'service'")
    assertTest(model.rootNode?.children?[1].key == "endpoints", "Second unwrapped child is 'endpoints'")
}

// 19. Test Format Options (4 spaces, Tab, alphabetical sorting)
do {
    let json = """
    {
      "zebra": 1,
      "apple": 2,
      "mango": 3
    }
    """
    let val = try JSONParser.parse(json)
    
    // Sort keys enabled
    let sorted = val.format(options: JSONFormatOptions(indentSpaces: 4, sortKeys: true, escapeSlashes: false))
    let expectedSorted = """
    {
        "apple": 2,
        "mango": 3,
        "zebra": 1
    }
    """
    assertTest(sorted == expectedSorted, "Format with sortKeys: true orders keys alphabetically")
    
    // Tabs indentation
    let tabs = val.format(options: JSONFormatOptions(indentSpaces: -1, sortKeys: false, escapeSlashes: false))
    assertTest(tabs.contains("{\n\t\"zebra\": 1"), "Format with indentSpaces: -1 uses tab indentation")
} catch {
    assertTest(false, "Failed to test format options: \(error)")
}

// 20. Test Model Transformations (beautifyText, minifyText, stringifyText, unescapeText)
do {
    let model = JSONDocumentModel()
    model.rawText = "{\"b\":2,\"a\":1}"
    
    // Beautify
    model.settings.indentSpaces = 2
    model.settings.sortKeysAlphabetically = true
    model.beautifyText()
    assertTest(model.rawText.contains("\"a\": 1"), "Beautify formats and sorts keys")
    
    // Minify
    model.minifyText()
    assertTest(model.rawText == "{\"a\":1,\"b\":2}", "Minify strips whitespace")
    
    // Stringify
    model.stringifyText()
    assertTest(model.rawText.hasPrefix("\"") && model.rawText.hasSuffix("\""), "Stringify wraps in quotes")
    assertTest(model.rawText.contains("\\\"a\\\":1"), "Stringify escapes internal quotes")
    
    // Unescape
    model.unescapeText()
    assertTest(!model.rawText.hasPrefix("\"{\\\""), "Unescape restores formatted JSON")
    assertTest(model.parseError == nil, "Model has no parse error after unescape")
}

// 21. Test First-Time Tab Switch Reflection Bugfix (selectTab synchronously parses dirty input)
do {
    let model = JSONDocumentModel()
    let initialVersion = model.treeVersion
    model.rawText = "{\"title\": \"Original Title\", \"count\": 1}"
    model.selectTab(.viewer)
    
    assertTest(model.activeTab == .viewer, "Active tab is viewer")
    assertTest(model.rootNode != nil, "Root node exists")
    assertTest(model.treeVersion == initialVersion + 1, "Tree version incremented after parsing initial text")
    let originalTitleRow = model.visibleTreeRows.first(where: { $0.node.key == "title" })
    assertTest(originalTitleRow?.node.value == .string("Original Title"), "Original title is parsed")
    assertTest(originalTitleRow?.id == "$.title", "Row id is the node path")
    
    // Switch to Text tab and edit input
    model.selectTab(.text)
    assertTest(model.activeTab == .text, "Switched to text tab")
    model.rawText = "{\"title\": \"Updated Title\", \"count\": 2, \"newField\": true}"
    assertTest(model.isDirty == true, "Model is dirty after text change")
    
    // Switch to Viewer tab on FIRST ATTEMPT
    model.selectTab(.viewer)
    
    // Changes MUST be reflected immediately on first switch, without needing a second switch!
    assertTest(model.activeTab == .viewer, "Active tab is viewer on first switch")
    assertTest(model.isDirty == false, "Model is not dirty after first switch to viewer")
    assertTest(model.treeVersion == initialVersion + 2, "Tree version incremented on first switch")
    let updatedTitleRow = model.visibleTreeRows.first(where: { $0.node.key == "title" })
    assertTest(updatedTitleRow?.node.value == .string("Updated Title"), "Updated title reflected on FIRST switch")
    // Row identity must not change when the tree is rebuilt: if it did, SwiftUI would
    // tear the whole list down and lose the scroll position.
    assertTest(updatedTitleRow?.id == "$.title", "Row id is unchanged by a rebuild")
    assertTest(updatedTitleRow?.id == originalTitleRow?.id, "The same node keeps the same row id across rebuilds")
    
    let newFieldRow = model.visibleTreeRows.first(where: { $0.node.key == "newField" })
    assertTest(newFieldRow?.node.value == .bool(true), "New field reflected on FIRST switch")
    
    // Test third modification
    model.selectTab(.text)
    model.rawText = "{\"title\": \"Third Version\"}"
    model.selectTab(.viewer)
    assertTest(model.treeVersion == initialVersion + 3, "Tree version incremented to third version")
    let thirdTitleRow = model.visibleTreeRows.first(where: { $0.node.key == "title" })
    assertTest(thirdTitleRow?.node.value == .string("Third Version"), "Third version reflected on first switch")
}

// 22. Test activeTab.didSet Direct Assignment
do {
    let model = JSONDocumentModel()
    let initialVersion = model.treeVersion
    model.rawText = "{\"status\": \"idle\"}"
    model.activeTab = .viewer
    assertTest(model.treeVersion == initialVersion + 1, "Direct activeTab assignment parsed tree")
    let statusRow = model.visibleTreeRows.first(where: { $0.node.key == "status" })
    assertTest(statusRow?.node.value == .string("idle"), "Status row contains idle")
    
    model.activeTab = .text
    model.rawText = "{\"status\": \"active\"}"
    assertTest(model.isDirty == true, "Model is dirty")
    model.activeTab = .viewer
    assertTest(model.treeVersion == initialVersion + 2, "Direct activeTab assignment parsed new treeVersion")
    let updatedStatusRow = model.visibleTreeRows.first(where: { $0.node.key == "status" })
    assertTest(updatedStatusRow?.node.value == .string("active"), "Status row reflects active on direct switch")
}

// 23. Test PythonLiteralParser
do {
    let pythonCode = """
    # Python dictionary with comments and trailing commas
    {
        'app_name': 'JSONViewer',
        'is_active': True,
        'is_guest': False,
        'cache': None,
        'ports': (8080, 8443,),
        'limits': {
            'max_mb': 100,
            'timeout_sec': 30,
        },
    }
    """
    do {
        let val = try PythonLiteralParser.parse(pythonCode)
        if case .object(let props) = val {
            assertTest(props.count == 6, "Python dict parsed into 6 properties")
            assertTest(props.first(where: { $0.key == "app_name" })?.value == .string("JSONViewer"), "app_name string parsed")
            assertTest(props.first(where: { $0.key == "is_active" })?.value == .bool(true), "True parsed as true")
            assertTest(props.first(where: { $0.key == "is_guest" })?.value == .bool(false), "False parsed as false")
            assertTest(props.first(where: { $0.key == "cache" })?.value == .null, "None parsed as null")
            assertTest(props.first(where: { $0.key == "ports" })?.value == .array([.number(8080, raw: "8080"), .number(8443, raw: "8443")]), "Tuple parsed as array")
        } else {
            assertTest(false, "Expected object from Python dict")
        }
    } catch {
        assertTest(false, "Failed to parse Python literal: \(error)")
    }
}

// 24. Test Python Dictionary Auto-Conversion to JSON in Model
do {
    let model = JSONDocumentModel()
    let pythonInput = "{'service': 'auth', 'enabled': True, 'tokens': None}"
    model.rawText = pythonInput
    
    // Test parseAndBuildTree auto-converts rawText to valid JSON
    let success = model.parseAndBuildTree(silent: false)
    assertTest(success == true, "parseAndBuildTree succeeded on Python dict input")
    assertTest(model.rawText.contains("\"service\": \"auth\""), "rawText auto-converted to double quotes")
    assertTest(model.rawText.contains("\"enabled\": true"), "rawText auto-converted True to true")
    assertTest(model.rawText.contains("\"tokens\": null"), "rawText auto-converted None to null")
    assertTest(model.parseError == nil, "No parse error after auto-conversion")
    
    // Test beautifyText auto-converts Python dict
    model.rawText = "{'debug': False, 'workers': 4}"
    model.beautifyText()
    assertTest(model.rawText.contains("\"debug\": false"), "beautifyText auto-converts Python dict to JSON")
    
    // Test convertPythonToJson
    model.rawText = "{'env': 'production', 'retries': 3}"
    model.convertPythonToJson()
    assertTest(model.rawText.contains("\"env\": \"production\""), "convertPythonToJson converts to JSON")
    
    // Test convertJsonToPython
    model.rawText = "{\"env\": \"staging\", \"active\": true, \"count\": null}"
    model.convertJsonToPython()
    assertTest(model.rawText.contains("'env': 'staging'"), "convertJsonToPython formats keys with single quotes")
    assertTest(model.rawText.contains("'active': True"), "convertJsonToPython converts true to True")
    assertTest(model.rawText.contains("'count': None"), "convertJsonToPython converts null to None")
    
    // Test Python variable assignment prefix (e.g. data = {...})
    model.rawText = "data = {'name': 'Alice', 'roles': ('admin', 'user')}"
    let varAssignSuccess = model.parseAndBuildTree(silent: false)
    assertTest(varAssignSuccess == true, "parseAndBuildTree succeeded with variable assignment prefix")
    assertTest(model.rawText.contains("\"name\": \"Alice\""), "Variable assignment stripped and converted to JSON")
    assertTest(model.rawText.contains("\"roles\": ["), "Python tuple in variable assignment converted to JSON array")
    
    // Test copyPythonObject
    model.copyPythonObject()
    assertTest(model.copiedToastMessage == "Copied Python Dictionary!", "Toast displays Copied Python Dictionary!")
}

// 25. Test Search Bar Focus Trigger
do {
    let model = JSONDocumentModel()
    assertTest(model.focusSearchFieldTrigger == 0, "Initial search focus trigger is 0")
    model.focusSearch()
    assertTest(model.isSearchVisible == true, "isSearchVisible is true after focusSearch()")
    assertTest(model.focusSearchFieldTrigger == 1, "focusSearchFieldTrigger incremented to 1")
    model.focusSearch()
    assertTest(model.focusSearchFieldTrigger == 2, "focusSearchFieldTrigger incremented to 2 on second call")
}

// 26. Editor text sync: the editor owns the text while typing, the model pulls it
// in only when something needs it.
do {
    let model = JSONDocumentModel()
    let staleText = model.rawText
    let source = FakeTextSource(text: "{\"typed\": 1}")
    model.registerTextSource(source)

    // One keystroke: the model must not copy the editor's text.
    model.markEditedFromEditor(lineCount: 1, characterCount: 12)
    assertTest(model.rawText == staleText, "An editor keystroke does not copy the document text")
    assertTest(model.hasPendingTextSync, "An editor keystroke marks the text as pending")
    assertTest(model.lineCount == 1 && model.characterCount == 12, "Editor-supplied metrics are used without a rescan")
    assertTest(model.isDirty, "An editor keystroke marks the document dirty")
    assertTest(model.isDocumentEmpty == false, "Editor metrics drive document emptiness")

    // Something that needs the text pulls it in, exactly once.
    let changed = model.syncRawTextIfNeeded()
    assertTest(changed, "syncRawTextIfNeeded copies the editor's text")
    assertTest(model.rawText == "{\"typed\": 1}", "The pulled-in text is the editor's text")
    assertTest(model.hasPendingTextSync == false, "The pending flag is cleared after a sync")
    assertTest(model.syncRawTextIfNeeded() == false, "A second sync does nothing")

    // A consumer such as Copy sees the edit even if it was never synced explicitly.
    source.text = "{\"typed\": 2}"
    model.markEditedFromEditor(lineCount: 1, characterCount: 12)
    model.copyText()
    assertTest(model.rawText == "{\"typed\": 2}", "Copy pulls in unsynced editor text")

    // Format must work off the editor's text, not the stale model copy.
    source.text = "{\"b\":2,\"a\":1}"
    model.markEditedFromEditor(lineCount: 1, characterCount: 13)
    model.beautifyText()
    assertTest(model.rawText.contains("\"a\": 1") && model.rawText.contains("\"b\": 2"), "Format operates on the editor's text")
    assertTest(model.hasPendingTextSync == false, "A transform clears the pending text flag")

    // The editor stays authoritative until it is told otherwise.
    source.text = "{\"b\":2,\"a\":1}"
    model.markEditedFromEditor(lineCount: 1, characterCount: 13)
    model.clearText()
    assertTest(model.hasPendingTextSync == false, "clearText supersedes unsynced editor text")
    assertTest(model.isDocumentEmpty, "clearText empties the document")

    // Registering a second editor must not shadow the one holding the edits.
    model.markEditedFromEditor(lineCount: 1, characterCount: 12)
    let idleSource = FakeTextSource(text: "stale", isActiveEditor: false)
    model.registerTextSource(idleSource)
    model.syncRawTextIfNeeded()
    assertTest(model.rawText == "{\"b\":2,\"a\":1}", "The editor that produced the last edit wins over a registered idle one")
    model.unregisterTextSource(idleSource)
}

// 26b. Text source lifetime: the model must never pull the wrong editor, and a
// deallocated editor must never leave a dangling reference.
do {
    let model = JSONDocumentModel()
    let editorA = FakeTextSource(text: "alpha")
    let editorB = FakeTextSource(text: "beta")
    model.registerTextSource(editorA)
    model.registerTextSource(editorB)

    // An edit lands in A, then focus moves to B before anything pulls the text. The
    // editor that produced the edit must win, or the keystroke is replaced by B's
    // older text.
    model.noteTextSourceEdited(editorA)
    editorA.text = "alpha-edited"
    model.markEditedFromEditor(lineCount: 1, characterCount: 12)
    editorB.isActiveEditor = true
    model.syncRawTextIfNeeded()
    assertTest(model.rawText == "alpha-edited", "The editing editor is preferred over the focused one")

    // With no editing editor, the focused one is used.
    editorA.isActiveEditor = false
    editorB.text = "beta-edited"
    model.noteTextSourceEdited(editorB)
    model.markEditedFromEditor(lineCount: 1, characterCount: 11)
    model.syncRawTextIfNeeded()
    assertTest(model.rawText == "beta-edited", "The focused editor is used when nothing was typed")

    // Registering the same source twice must not duplicate it.
    model.registerTextSource(editorB)
    model.registerTextSource(editorB)
    editorB.text = "beta-again"
    model.markEditedFromEditor(lineCount: 1, characterCount: 10)
    model.syncRawTextIfNeeded()
    assertTest(model.rawText == "beta-again", "Re-registering an editor is idempotent")

    // Unregistering flushes, so leaving the tab does not lose the last keystrokes.
    editorB.text = "unsaved"
    model.markEditedFromEditor(lineCount: 1, characterCount: 7)
    model.unregisterTextSource(editorB)
    assertTest(model.rawText == "unsaved", "Unregistering an editor flushes its pending text")
    assertTest(model.hasPendingTextSync == false, "Unregistering clears the pending flag")

    // A source that goes away without unregistering must not be read.
    var doomed: FakeTextSource? = FakeTextSource(text: "doomed")
    model.registerTextSource(doomed!)
    model.noteTextSourceEdited(doomed!)
    doomed!.text = "doomed-edited"
    model.markEditedFromEditor(lineCount: 1, characterCount: 13)
    doomed = nil
    // It may still legitimately sync from a live registered editor, but it must never
    // reach for the one that is gone.
    model.syncRawTextIfNeeded()
    assertTest(model.rawText != "doomed-edited", "A deallocated editor is never read")
    assertTest(model.hasPendingTextSync == false, "A dead source does not leave the text permanently unsynced")
}

// 26c. The text revision the editor uses to decide whether it needs to be updated at
// all. Getting this wrong means either not typing text back, or reverting an edit.
do {
    let model = JSONDocumentModel()
    let source = FakeTextSource(text: "{\"a\": 1}")
    model.registerTextSource(source)
    let base = model.modelTextRevision
    
    // An editor edit does not bump it: the buffer already has that text.
    model.noteTextSourceEdited(source)
    model.markEditedFromEditor(lineCount: 1, characterCount: 8)
    assertTest(model.modelTextRevision == base, "An editor edit does not bump the model text revision")
    model.syncRawTextIfNeeded()
    assertTest(model.modelTextRevision == base, "Syncing the editor's text does not bump the revision")
    
    // Everything that produces new text from the model does.
    model.rawText = "{\"a\": 2}"
    assertTest(model.modelTextRevision == base, "A direct assignment outside setRawTextFromModel is not expected")
    
    model.beautifyText()
    let afterBeautify = model.modelTextRevision
    assertTest(afterBeautify > base, "A transform bumps the model text revision")
    
    model.minifyText()
    assertTest(model.modelTextRevision > afterBeautify, "Another transform bumps it again")
    
    model.clearText()
    assertTest(model.modelTextRevision > 0, "Clearing bumps the model text revision")
    model.unregisterTextSource(source)
}

// 27. Emptying the document drops the stale tree, selection and error.
do {
    let model = JSONDocumentModel()
    model.rawText = "{\"a\":1}"
    _ = model.parseAndBuildTree(silent: true)
    assertTest(model.rootNode != nil, "Tree built for the first document")
    model.toggleExpand(nodeId: "$.a")
    model.searchQuery = "a"

    model.rawText = "{ broken"
    _ = model.parseAndBuildTree(silent: true)
    assertTest(model.parseError != nil, "An invalid document records a parse error")
    assertTest(model.rootNode != nil, "An invalid but non-empty document keeps the last good tree")

    model.rawText = ""
    let versionBefore = model.treeVersion
    assertTest(model.parseAndBuildTree(silent: true) == false, "An empty document does not parse")
    assertTest(model.rootNode == nil, "Emptying the document drops the stale tree")
    assertTest(model.jsonValue == nil, "Emptying the document drops the parsed value")
    assertTest(model.selectedNode == nil, "Emptying the document drops the selection")
    assertTest(model.selectedNodeProperties.isEmpty, "Emptying the document drops the property grid")
    assertTest(model.visibleTreeRows.isEmpty, "Emptying the document drops the visible rows")
    assertTest(model.expandedNodeIds.isEmpty && model.expandedLeafNodeIds.isEmpty, "Emptying the document drops the expanded sets")
    assertTest(model.parseError == nil, "Emptying the document clears the stale error")
    assertTest(model.searchResults.isEmpty && model.searchQuery.isEmpty, "Emptying the document clears search")
    assertTest(model.treeVersion > versionBefore, "Discarding the tree bumps the version so old rows cannot be reused")

    // Whitespace only counts as empty.
    model.rawText = "{\"a\":1}"
    _ = model.parseAndBuildTree(silent: true)
    model.rawText = "   \n\t  \n "
    _ = model.parseAndBuildTree(silent: true)
    assertTest(model.rootNode == nil, "A whitespace-only document drops the stale tree")
}

// 27b. Metrics: the editor's counts must not be reused for a different document,
// and a stale in-flight measurement must not overwrite them.
do {
    let model = JSONDocumentModel()
    let source = FakeTextSource(text: "one\ntwo\nthree")
    model.registerTextSource(source)

    // An editor edit counts 3 lines / 13 characters.
    model.markEditedFromEditor(lineCount: 3, characterCount: 13)
    assertTest(model.lineCount == 3 && model.characterCount == 13, "Editor counts are adopted")

    // The sync that pulls that same text in keeps them, with no rescan.
    model.syncRawTextIfNeeded()
    assertTest(model.lineCount == 3 && model.characterCount == 13, "Syncing the editor's text keeps its counts")

    // Now the model takes the text from somewhere else: the editor's counts described
    // different text, so they must not be carried over.
    model.markEditedFromEditor(lineCount: 3, characterCount: 13)
    model.rawText = "{\"a\":1}\n{\"b\":2}"
    assertTest(model.lineCount == 2 && model.characterCount == 15, "A model-side edit is measured, not given the editor's counts")

    // Emptying the document through the editor is reflected immediately.
    source.text = ""
    model.noteTextSourceEdited(source)
    model.markEditedFromEditor(lineCount: 1, characterCount: 0)
    assertTest(model.isDocumentEmpty, "An editor deletion reports an empty document")
    model.unregisterTextSource(source)

    // An editor edit on a document too large to measure synchronously: the pending
    // background measurement of the previous text must not land on top of it, and the
    // idle rebuild that syncs the editor must not invent different numbers.
    let big = String(repeating: "x\n", count: 20_000)
    let bigSource = FakeTextSource(text: big)
    model.registerTextSource(bigSource)
    model.noteTextSourceEdited(bigSource)
    model.rawText = String(repeating: "y\n", count: 20_000)
    model.markEditedFromEditor(lineCount: 20_001, characterCount: (big as NSString).length)
    // The document is still the "y" text: the editor is ahead of the model.
    assertTest(model.rawText.hasPrefix("y\n"), "The editor's text has not been pulled in yet")
    // Wait for the rebuild itself, not just for the in-flight flag, which is still
    // false while the debounce is running.
    await waitFor("the idle rebuild to sync the editor and finish", timeout: 20.0) {
        model.rawText == big && model.isParsing == false
    }
    assertTest(model.rawText == big, "The idle rebuild synced the editor's text")
    assertTest(model.lineCount == 20_001 && model.characterCount == (big as NSString).length, "An editor edit is not overwritten by a stale background measurement")
    model.unregisterTextSource(bigSource)
}

// 28. The editor's incremental line count must match a full scan for every kind of
// edit the editor can produce. This is the arithmetic the Coordinator performs in
// `shouldChangeTextIn`, exercised here without a window server.
do {
    func truthLineCount(_ bytes: [UInt8]) -> Int {
        bytes.isEmpty ? 1 : 1 + bytes.filter { $0 == 0x0A }.count
    }
    func lineCount(_ text: String) -> Int { JSONDocumentModel.lineCount(of: text) }
    func newlineCount(_ text: String) -> Int { lineCount(text) - 1 }

    let base = "{\n  \"a\": 1,\n  \"b\": [1, 2]\n}"
    let baseBytes = Array(base.utf8)

    // replay a sequence of (replaced range, replacement) edits, tracking lines the
    // same way the editor delegate does
    func replay(_ edits: [(Range<Int>, String)], initial: String) -> (incremental: Int, truth: Int) {
        var bytes = Array(initial.utf8)
        var tracked = truthLineCount(bytes)
        for (range, replacement) in edits {
            let removed = newlineCount(String(decoding: bytes[range], as: UTF8.self))
            let added = newlineCount(replacement)
            tracked = max(1, tracked + added - removed)
            bytes.replaceSubrange(range, with: Array(replacement.utf8))
        }
        return (tracked, truthLineCount(bytes))
    }

    func check(_ name: String, _ edits: [(Range<Int>, String)], initial: String = base) {
        let (got, want) = replay(edits, initial: initial)
        assertTest(got == want, "Incremental line count: \(name) (\(got) vs \(want))")
    }

    let end = baseBytes.count..<baseBytes.count
    check("append a character", [(end, "x")])
    check("delete a newline", [(1..<2, "")])
    check("Enter inserts a line", [(end, "\n")])
    check("paste a multi-line block", [(end, "\n{\n  \"c\": 3\n}\n")])
    check("select all and delete", [(0..<baseBytes.count, "")])
    check("delete everything but a character", [(0..<baseBytes.count, " ")])
    check("replace a newline with a newline", [(1..<2, "\n")])
    check("delete then retype", [(1..<2, ""), (1..<1, "\n")])
    check("replace a range spanning lines", [(1..<8, "X")])
    check("append several lines at once", [(end, "\n\n\n")])

    // Typing into an empty buffer, and an entirely empty document.
    check("first character into an empty document", [(0..<0, "a")], initial: "")
    check("newlines into an empty document", [(0..<0, "\n\n\n")], initial: "")
    check("deleting the only character", [(0..<1, "")], initial: "a")

    // Every case agrees with the real counter as well as the byte scan.
    for text in [base, "", "a", "\n", "a\nb", "árvíztűrő\ntükör"] {
        assertTest(lineCount(text) == truthLineCount(Array(text.utf8)), "Line counter agrees with a byte scan for \(text.debugDescription)")
    }
}

// 29. The live-parse debounce scales with the document, and the line counter
// matches a plain byte-by-byte count.
do {
    let model = JSONDocumentModel()
    model.rawText = ""
    assertTest(model.characterCount == 0, "Empty document reports no characters")
    assertTest(model.rebuildDebounceDelay == 0.35, "Empty document uses the base 350 ms debounce")

    // 350 ms up to 1 MB, then +250 ms per MB.
    model.rawText = String(repeating: "x", count: 512 * 1024)
    assertTest(model.rebuildDebounceDelay == 0.475, "Debounce grows by 250 ms per MB")

    model.rawText = String(repeating: "x", count: 1024 * 1024)
    assertTest(model.rebuildDebounceDelay == 0.6, "Debounce reaches 600 ms at 1 MB")

    model.rawText = String(repeating: "x", count: 40 * 1024 * 1024)
    assertTest(model.rebuildDebounceDelay == 1.5, "Debounce is capped at 1.5 s")

    // The character count must be available synchronously even for a document large
    // enough that its line count is measured off the main thread, because that count
    // is what sizes the debounce.
    model.rawText = String(repeating: "{\n", count: 30_000)
    assertTest(model.characterCount == 60_000, "Character count is measured synchronously on a large document")
    assertTest(model.rebuildDebounceDelay > 0.35, "A large document gets a longer debounce immediately")

    var builder = ""
    for i in 0..<5000 { builder += "line \(i)\n" }
    assertTest(JSONDocumentModel.lineCount(of: builder) == 5001, "Line counter counts a trailing newline as a new line")
    assertTest(JSONDocumentModel.lineCount(of: "") == 1, "Empty text is one line")
    assertTest(JSONDocumentModel.lineCount(of: "a\r\nb") == 2, "CRLF counts as one line break")
    assertTest(JSONDocumentModel.lineCount(of: "no newline") == 1, "Text without newlines is one line")
    assertTest(JSONDocumentModel.lineCount(of: "a\n\n\n") == 4, "Consecutive newlines each add a line")
    let utf8Text = "árvíztűrő tükörfúrógép\nárvíztűrő tükörfúrógép"
    assertTest(JSONDocumentModel.lineCount(of: utf8Text) == 2, "Line counter handles multi-byte UTF-8")

    // A chunk bigger than the internal scan chunk must still be counted exactly.
    let big = String(repeating: "0123456789abcdef\n", count: 300_000)
    assertTest(JSONDocumentModel.lineCount(of: big) == 300_001, "Line counter is correct beyond one scan chunk")
}

// 29. Split-mode live parse must not resurrect a cleared document or apply a stale result.
do {
    let model = JSONDocumentModel()
    model.rawText = "{\"keep\":1}"
    _ = model.parseAndBuildTree(silent: true)
    model.activeTab = .split
    assertTest(model.rootNode != nil, "Split tab has a tree to work from")

    model.clearText()
    assertTest(model.rootNode == nil, "Clearing in Split mode drops the tree straight away")
    assertTest(model.isDocumentEmpty, "Clearing in Split mode marks the document empty")
}

// 30. A tab switch on a large document must not block: the rebuild moves off the
// main thread and is published when it lands.
do {
    // Comfortably past the 256K character inline limit, but small enough to keep the
    // suite quick.
    let big = (0..<6_000).map { "{\"id\": \"id-\($0)\", \"name\": \"user_\($0)\", \"tags\": [\"a\", \"b\"]}" }
        .joined(separator: ",\n")
    let bigJSON = "[\n\(big)\n]"
    assertTest(JSONDocumentModel.exceedsInlineParseLimit(bigJSON), "A document past the inline limit is parsed off the main thread")
    assertTest(!JSONDocumentModel.exceedsInlineParseLimit("{\"a\": 1}"), "A small document is still parsed inline")

    let model = JSONDocumentModel()
    model.rawText = "{\"title\": \"first\"}"
    _ = model.parseAndBuildTree(silent: true)
    assertTest(model.isParsing == false, "An inline rebuild reports no background parse")

    // A dirty tab switch onto a large document.
    model.rawText = bigJSON
    let versionBefore = model.treeVersion
    model.selectTab(.viewer)
    assertTest(model.isParsing, "A large rebuild reports that it is in flight")
    assertTest(model.activeTab == .viewer, "The tab switches immediately even while the rebuild is in flight")

    await waitFor("the background rebuild to finish", timeout: 20.0) { model.isParsing == false }
    assertTest(model.isParsing == false, "The background rebuild clears its flag")
    assertTest(model.treeVersion == versionBefore + 1, "The background rebuild is applied")
    assertTest(model.rootNode?.children?.count == 6_000, "The rebuilt tree has every item")
    assertTest(model.isDirty == false, "The rebuilt tree clears the dirty flag")
    assertTest(model.parseError == nil, "The rebuilt tree has no parse error")

    // A stale result must be discarded rather than applied: edit again while the
    // first rebuild is still running.
    model.selectTab(.text)
    model.rawText = "[\n\(big)\n]"
    model.selectTab(.viewer)
    model.rawText = bigJSON
    await waitFor("the superseded rebuilds to settle", timeout: 30.0) { model.isParsing == false }
    assertTest(model.rootNode?.children?.count == 6_000, "A superseded rebuild is not applied on top of the newest one")
    assertTest(model.isParsing == false, "isParsing never gets stuck after superseded rebuilds")

    // Emptying a large document while a rebuild is in flight must still clear the tree.
    model.selectTab(.text)
    model.rawText = bigJSON
    model.selectTab(.viewer)
    await waitFor("the rebuild to finish", timeout: 20.0) { model.isParsing == false }
    model.clearText()
    assertTest(model.rootNode == nil, "Clearing after a background rebuild still drops the tree")
    assertTest(model.isParsing == false, "Clearing does not leave a parse in flight")
}

do {
    // 30b. Opening a file reads and rebuilds off the main thread, and a file that is
    // cleared or superseded while being read never lands.
        let model = JSONDocumentModel()
        let big = (0..<6_000).map { "{\"id\": \"id-\($0)\", \"name\": \"user_\($0)\"}" }.joined(separator: ",\n")
        let dir = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("jsonviewer-tests-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
    
        let goodURL = dir.appendingPathComponent("good.json")
        let fileText = "{\"title\": \"opened\", \"items\": [\(big)]}"
        try fileText.write(to: goodURL, atomically: true, encoding: .utf8)
    
        model.openFile(url: goodURL)
        // The text is not there yet: the read is still in flight.
        let fileContents = try String(contentsOf: goodURL, encoding: .utf8)
        assertTest(model.rawText != fileContents, "Opening a file does not block on the read")
        await waitFor("the file to be read and rebuilt", timeout: 20.0) {
            model.rawText.contains("\"title\": \"opened\"") && model.isParsing == false
        }
        assertTest(model.rootNode?.key == "JSON", "The opened document is parsed")
        assertTest(model.rootNode?.children?.first?.key == "title", "The opened document has the expected shape")
    
        // Clearing while a file is being read must win.
        let slowURL = dir.appendingPathComponent("slow.json")
        try "{\"title\": \"should never appear\"}".write(to: slowURL, atomically: true, encoding: .utf8)
        model.openFile(url: slowURL)
        model.clearText()
        try? await Task.sleep(nanoseconds: 300_000_000)
        assertTest(model.rawText.isEmpty, "A file still being read does not overwrite a cleared document")
        assertTest(model.rootNode == nil, "A cleared document stays cleared while a file is in flight")
    
        // A missing file reports the failure instead of silently doing nothing.
        let model2 = JSONDocumentModel()
        model2.openFile(url: dir.appendingPathComponent("does-not-exist.json"))
        await waitFor("the missing file to be reported", timeout: 10.0) { model2.isErrorAlertPresented }
        assertTest(model2.isErrorAlertPresented, "A missing file raises an error")
        assertTest(model2.errorMessage.contains("Failed to open file"), "The error explains what failed")
} catch {
    assertTest(false, "File-open tests failed to run: \(error)")
}

// 31. Rebuilding carries the navigation state over without indexing the whole tree.
do {
    let model = JSONDocumentModel()
    let json = """
    {
      "store": {
        "book": [
          { "title": "Swift", "price": 49.99, "tags": ["a", "b"] },
          { "title": "Rust", "price": 39.99, "tags": ["c"] }
        ]
      }
    }
    """
    model.rawText = json
    _ = model.parseAndBuildTree(silent: true)

    // root -> store -> book (array) -> first item -> its properties
    let bookArray = model.rootNode!.children![0].children![0]
    let book = bookArray.children![0]
    let price = book.children!.first { $0.key == "price" }!
    let tag = book.children!.first { $0.key == "tags" }!
    model.toggleExpand(nodeId: bookArray.id)
    model.toggleExpand(nodeId: book.id)
    model.selectAndReveal(node: tag)
    let rowsBefore = model.visibleTreeRows.count

    // Rebuild with unrelated edits elsewhere; the paths still resolve.
    model.rawText = json.replacingOccurrences(of: "\"price\": 49.99", with: "\"price\": 59.99")
    _ = model.parseAndBuildTree(silent: true)

    assertTest(model.selectedNode?.path == "$.store.book[0].tags", "A deep selection survives a rebuild")
    assertTest(model.expandedNodeIds.contains(bookArray.id) && model.expandedNodeIds.contains(book.id), "Expanded containers survive a rebuild")
    assertTest(model.visibleTreeRows.count == rowsBefore, "The rebuilt tree has the same shape")
    let rebuiltPrice = model.rootNode!.children![0].children![0].children![0].children!.first { $0.key == "price" }
    assertTest(rebuiltPrice?.valueString == "59.99", "The rebuilt tree shows the new value")

    // A selected leaf that the property grid expands keeps its state too.
    model.toggleExpandLeaf(nodeId: price.id)
    _ = model.parseAndBuildTree(silent: true)
    assertTest(model.expandedLeafNodeIds.contains(price.id), "An expanded leaf survives a rebuild")

    // Now remove the selected branch entirely: the state must be dropped, not restored
    // onto whatever node happens to sit at that path now.
    model.rawText = "{\"store\": 5}"
    _ = model.parseAndBuildTree(silent: true)
    assertTest(model.selectedNode?.path == "$", "A selection that no longer exists falls back to the root")
    assertTest(model.expandedNodeIds.count == 1, "Expansion of nodes that no longer exist is dropped")
    // The root stays expanded, so it and its one remaining child are the only rows.
    assertTest(model.visibleTreeRows.count == 2, "Only the root and its child are left after everything vanished")
    assertTest(model.expandedLeafNodeIds.isEmpty, "Leaf expansion of nodes that no longer exists is dropped")
}

// 32. A Python literal is one value: junk must not parse, and must never rewrite the
// document. This was destroying whatever the user had pasted.
do {
    assertTest((try? PythonLiteralParser.parse("x\nx\nx\n")) == nil, "A bare word followed by junk is not a Python literal")
    assertTest((try? PythonLiteralParser.parse("hello")) != nil, "A bare word on its own is still a Python string")
    assertTest((try? PythonLiteralParser.parse("{'a': 1} trailing")) == nil, "Trailing content after a Python dict is rejected")
    assertTest((try? PythonLiteralParser.parse("{'a': 1}")) != nil, "A complete Python dict still parses")
    assertTest((try? PythonLiteralParser.parse("# just a comment\n{'a': 1}")) != nil, "Comments around a Python dict are still fine")
    
    // The document itself must survive a rebuild of junk.
    let junk = String(repeating: "x\n", count: 5_000)
    let model = JSONDocumentModel()
    let source = FakeTextSource(text: junk)
    model.registerTextSource(source)
    model.noteTextSourceEdited(source)
    let beforeJunk = model.rebuildCount
    model.markEditedFromEditor(lineCount: 5_000, characterCount: (junk as NSString).length)
    await waitForRebuild(model, from: beforeJunk)
    assertTest(model.rawText == junk, "Junk is never rewritten to a quoted string")
    
    // A word the user is still typing must not be rewritten either.
    let typing = FakeTextSource(text: "hello")
    model.registerTextSource(typing)
    model.noteTextSourceEdited(typing)
    let beforeWord = model.rebuildCount
    model.markEditedFromEditor(lineCount: 1, characterCount: 5)
    await waitForRebuild(model, from: beforeWord)
    assertTest(model.rawText == "hello", "A bare word being typed is left exactly as typed")
    
    // The feature that should still work: a Python dict is converted to JSON.
    let dict = FakeTextSource(text: "{'a': 1, 'b': True}")
    model.registerTextSource(dict)
    model.noteTextSourceEdited(dict)
    let beforeDict = model.rebuildCount
    model.markEditedFromEditor(lineCount: 1, characterCount: (dict.text as NSString).length)
    await waitForRebuild(model, from: beforeDict)
    assertTest(model.rawText.contains("\"a\": 1") && model.rawText.contains("\"b\": true"), "A Python dict is still converted to JSON")
    model.unregisterTextSource(dict)
}

// 33. Tab switches preserve editor cursor/scroll state, and clearing resets them.
do {
    let model = JSONDocumentModel()
    model.rawText = "{\n  \"hello\": \"world\"\n}"
    assertTest(model.lastEditorSelectedRange == nil, "Editor selected range initially nil")
    assertTest(model.lastEditorScrollOrigin == nil, "Editor scroll origin initially nil")
    
    // Simulate setting selection and scroll
    model.lastEditorSelectedRange = NSRange(location: 4, length: 5)
    model.lastEditorScrollOrigin = CGPoint(x: 0, y: 120)
    assertTest(model.lastEditorSelectedRange?.location == 4, "Editor selection preserved")
    assertTest(model.lastEditorScrollOrigin?.y == 120, "Editor scroll origin preserved")
    
    // Switch tabs
    model.selectTab(.viewer)
    assertTest(model.activeTab == .viewer, "Active tab is viewer")
    assertTest(model.lastEditorSelectedRange?.location == 4, "Editor selection survives tab switch to viewer")
    
    model.selectTab(.split)
    assertTest(model.activeTab == .split, "Active tab is split")
    assertTest(model.lastEditorScrollOrigin?.y == 120, "Editor scroll survives tab switch to split")
    
    // Clearing resets the editor cursor and scroll state
    model.clearText()
    assertTest(model.lastEditorSelectedRange?.location == 0, "Clearing resets editor cursor to top")
    assertTest(model.lastEditorScrollOrigin == .zero, "Clearing resets editor scroll to zero")
}

// 34. Benchmark: 5MB JSON edit and tab switch
do {
    let count = 40_000
    let repeatedPayload = String(repeating: "x", count: 59)
    let items = (0..<count).map {
        "{\"id\": \($0), \"name\": \"item_\($0)\", \"value\": \($0 * 2), \"payload\": \"\(repeatedPayload)\"}"
    }.joined(separator: ",\n")
    let json = "[\n" + items + "\n]"
    print("\n--- 5MB Benchmark ---")
    print("Document size: \(Double(json.utf8.count) / (1024.0 * 1024.0)) MB (\(json.utf8.count) bytes)")
    assertTest(json.utf8.count >= 5_000_000, "Benchmark exercises at least 5 MB of JSON")
    
    let model = JSONDocumentModel()
    model.rawText = json
    let t0 = CFAbsoluteTimeGetCurrent()
    model.parseAndBuildTree(silent: true)
    let t1 = CFAbsoluteTimeGetCurrent()
    print("Initial parseAndBuildTree: \(String(format: "%.4f s", t1 - t0))")
    print("Initial visible rows: \(model.visibleTreeRows.count)")
    
    // Simulate user editing 3rd row while in Text tab:
    model.selectTab(.text)
    let editedJSON = json.replacingOccurrences(of: "\"name\": \"item_2\"", with: "\"name\": \"edited_2\"")
    let fakeSource = FakeTextSource(text: editedJSON)
    model.registerTextSource(fakeSource)
    model.noteTextSourceEdited(fakeSource)
    model.markEditedFromEditor(lineCount: count + 2, characterCount: (fakeSource.text as NSString).length)
    
    // Now switch to Viewer tab:
    let t2 = CFAbsoluteTimeGetCurrent()
    let prevRebuildCount = model.rebuildCount
    model.selectTab(.viewer)
    let t3 = CFAbsoluteTimeGetCurrent()
    print("selectTab(.viewer) synchronous time: \(String(format: "%.4f s", t3 - t2))")
    print("isParsing right after selectTab: \(model.isParsing)")
    
    // Wait for background parse to land:
    await waitForRebuild(model, from: prevRebuildCount)
    let t4 = CFAbsoluteTimeGetCurrent()
    print("Total time until rebuild applied: \(String(format: "%.4f s", t4 - t2))")
    print("Rebuilt visible rows: \(model.visibleTreeRows.count)")
    
    assertTest(model.visibleTreeRows.count == count + 1, "5MB tree has all rows")
    assertTest(model.selectedNodeProperties.count == count, "5MB root properties are prepared for the grid")
    assertTest(model.findNode(byId: "$[2].name")?.valueString == "edited_2", "The third-row edit is in the rebuilt tree")
    assertTest(model.isDirty == false, "5MB tree is clean after rebuild")
    model.unregisterTextSource(fakeSource)
}

// 35. Test rebuildTreeIfNeeded is non-blocking on dirty large documents and precomputes rows
print("\n--- Test 35: rebuildTreeIfNeeded non-blocking with precomputed rows ---")
do {
    let model = JSONDocumentModel()
    // Generate a 1MB+ JSON
    var items: [String] = []
    for i in 0..<15000 {
        items.append("{\"id\":\(i),\"tag\":\"test_\(i)\"}")
    }
    let bigJSON = "[\n" + items.joined(separator: ",\n") + "\n]"
    let fakeSource = FakeTextSource(text: bigJSON)
    model.registerTextSource(fakeSource)
    model.noteTextSourceEdited(fakeSource)
    model.markEditedFromEditor(lineCount: 15002, characterCount: (bigJSON as NSString).length)
    assertTest(model.isDirty == true, "Model is dirty after editor edit")
    
    let prevRebuildCount = model.rebuildCount
    let t0 = CFAbsoluteTimeGetCurrent()
    model.rebuildTreeIfNeeded(silent: true)
    let t1 = CFAbsoluteTimeGetCurrent()
    let elapsed = t1 - t0
    assertTest(elapsed < 0.05, "rebuildTreeIfNeeded returns immediately (\(String(format: "%.4f s", elapsed)))")
    assertTest(model.isParsing == true, "rebuildTreeIfNeeded moved work to background")
    
    await waitForRebuild(model, from: prevRebuildCount)
    assertTest(model.isParsing == false, "Background rebuild completed")
    assertTest(model.isDirty == false, "Model is clean after background rebuild")
    assertTest(model.visibleTreeRows.count == 15001, "Visible rows precomputed correctly (count: \(model.visibleTreeRows.count))")
    model.unregisterTextSource(fakeSource)
}

// 36. Test tab switching does not emit spurious navigation or scroll requests
print("\n--- Test 36: Tab switching does not emit spurious tree navigation requests ---")
do {
    let model = JSONDocumentModel()
    model.rawText = "{\"a\": 1, \"b\": 2, \"c\": 3}"
    model.parseAndBuildTree(silent: true)
    
    let navReqBefore = model.treeNavigationRequest
    let scrollReqBefore = model.treeScrollToTopRequest
    
    model.selectTab(.text)
    assertTest(model.activeTab == .text, "Switched to text tab")
    assertTest(model.treeNavigationRequest == navReqBefore, "No spurious nav request on text tab switch")
    assertTest(model.treeScrollToTopRequest == scrollReqBefore, "No spurious scroll-to-top request on text tab switch")
    
    model.selectTab(.viewer)
    assertTest(model.activeTab == .viewer, "Switched to viewer tab")
    assertTest(model.treeNavigationRequest == navReqBefore, "No spurious nav request on viewer tab switch")
    assertTest(model.treeScrollToTopRequest == scrollReqBefore, "No spurious scroll-to-top request on viewer tab switch")
    
    model.selectTab(.split)
    assertTest(model.activeTab == .split, "Switched to split tab")
    assertTest(model.treeNavigationRequest == navReqBefore, "No spurious nav request on split tab switch")
    assertTest(model.treeScrollToTopRequest == scrollReqBefore, "No spurious scroll-to-top request on split tab switch")
}

print("\n-----------------------------------------")
print("Total Tests: \(totalTests)")
print("Passed:      \(passedTests)")
print("Failed:      \(failedTests)")
print("-----------------------------------------\n")

    if failedTests > 0 {
        exit(1)
    } else {
        print("🎉 ALL TESTS PASSED SUCCESSFULLY!\n")
        exit(0)
    }
}

await runSuite()
