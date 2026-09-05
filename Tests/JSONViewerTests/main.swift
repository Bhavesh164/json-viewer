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
