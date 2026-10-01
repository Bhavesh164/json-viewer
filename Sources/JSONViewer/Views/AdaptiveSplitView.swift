import AppKit
import SwiftUI
import JSONViewerCore

public struct AdaptiveSplitView<Left: View, Right: View>: NSViewControllerRepresentable {
    public let activeTab: AppTab
    public let leftView: Left
    public let rightView: Right
    
    public init(activeTab: AppTab, leftView: Left, rightView: Right) {
        self.activeTab = activeTab
        self.leftView = leftView
        self.rightView = rightView
    }
    
    public func makeNSViewController(context: Context) -> AdaptiveSplitViewController {
        let leftHosting = NSHostingView(rootView: leftView)
        let rightHosting = NSHostingView(rootView: rightView)
        leftHosting.autoresizingMask = [.width, .height]
        rightHosting.autoresizingMask = [.width, .height]
        
        let vc = AdaptiveSplitViewController(leftView: leftHosting, rightView: rightHosting, initialTab: activeTab)
        context.coordinator.leftHosting = leftHosting
        context.coordinator.rightHosting = rightHosting
        context.coordinator.splitVC = vc
        return vc
    }
    
    public func updateNSViewController(_ nsViewController: AdaptiveSplitViewController, context: Context) {
        context.coordinator.leftHosting?.rootView = leftView
        context.coordinator.rightHosting?.rootView = rightView
        nsViewController.applyTab(activeTab)
    }
    
    public func makeCoordinator() -> Coordinator {
        Coordinator()
    }
    
    public class Coordinator {
        var leftHosting: NSHostingView<Left>?
        var rightHosting: NSHostingView<Right>?
        weak var splitVC: AdaptiveSplitViewController?
    }
}

public final class AdaptiveSplitViewController: NSSplitViewController {
    public private(set) var leftItem: NSSplitViewItem!
    public private(set) var rightItem: NSSplitViewItem!
    private var lastAppliedTab: AppTab?
    
    public init(leftView: NSView, rightView: NSView, initialTab: AppTab) {
        super.init(nibName: nil, bundle: nil)
        
        let leftVC = NSViewController()
        leftVC.view = leftView
        let rightVC = NSViewController()
        rightVC.view = rightView
        
        leftItem = NSSplitViewItem(viewController: leftVC)
        leftItem.canCollapse = true
        leftItem.holdingPriority = .defaultLow
        
        rightItem = NSSplitViewItem(viewController: rightVC)
        rightItem.canCollapse = true
        rightItem.holdingPriority = .defaultLow
        
        self.addSplitViewItem(leftItem)
        self.addSplitViewItem(rightItem)
        self.splitView.isVertical = true
        self.splitView.dividerStyle = .thin
        
        applyTab(initialTab, animated: false)
    }
    
    public required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    public func applyTab(_ tab: AppTab, animated: Bool = false) {
        guard lastAppliedTab != tab else { return }
        lastAppliedTab = tab
        
        let performCollapse = {
            switch tab {
            case .text:
                self.leftItem.isCollapsed = false
                self.rightItem.isCollapsed = true
            case .viewer:
                self.leftItem.isCollapsed = true
                self.rightItem.isCollapsed = false
            case .split:
                self.leftItem.isCollapsed = false
                self.rightItem.isCollapsed = false
                let width = self.splitView.bounds.width
                if width > 600 {
                    let currentPos = self.splitView.subviews.first?.frame.width ?? 0
                    if currentPos < 200 || currentPos > width - 200 {
                        self.splitView.setPosition(width / 2, ofDividerAt: 0)
                    }
                }
            }
            self.splitView.layoutSubtreeIfNeeded()
        }
        
        if animated {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.15
                context.allowsImplicitAnimation = true
                performCollapse()
            }
        } else {
            performCollapse()
        }
    }
}
