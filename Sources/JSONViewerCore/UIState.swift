import SwiftUI

/// A clean wrapper around SwiftUI's `State` property wrapper that avoids invoking compiler macro expansions
/// when building with Command Line Tools where `SwiftUIMacros` dylib is omitted.
@propertyWrapper
public struct UIState<T>: DynamicProperty {
    private var state: State<T>
    
    public init(wrappedValue: T) {
        self.state = State(initialValue: wrappedValue)
    }
    
    public var wrappedValue: T {
        get { state.wrappedValue }
        nonmutating set { state.wrappedValue = newValue }
    }
    
    public var projectedValue: Binding<T> {
        state.projectedValue
    }
}
