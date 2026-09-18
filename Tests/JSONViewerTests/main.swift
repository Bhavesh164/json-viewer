import Foundation
import JSONViewerCore

var totalTests = 0
var passedTests = 0
var failedTests = 0

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
    assertTest(model.searchResults.count == 2, "New search 'admin' finds 2 matches")
    assertTest(model.currentSearchIndex == 0, "New search resets to index 0")
    assertTest(model.searchStatus == "1 of 2 matches", "Status displays '1 of 2 matches'")
    
    // Query change: non-existent phrase
    model.searchQuery = "nonexistentxyz"
    model.searchSubmit(reverse: false)
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
    assertTest(originalTitleRow?.id == "\(initialVersion + 1):$.title", "Row id contains treeVersion")
    
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
    assertTest(updatedTitleRow?.id == "\(initialVersion + 2):$.title", "Row id contains new treeVersion")
    
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
