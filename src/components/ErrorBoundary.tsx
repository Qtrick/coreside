import { Component, type ErrorInfo, type ReactNode } from "react";

type Props = {
  children: ReactNode;
};

type State = {
  error: Error | null;
};

/**
 * Root recoverable boundary so chat/settings crashes do not white-screen the app.
 * Tool surfaces keep their own ToolErrorBoundary.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("Coreside UI error", error, info.componentStack);
  }

  private reload = () => {
    window.location.reload();
  };

  private dismiss = () => {
    this.setState({ error: null });
  };

  render() {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <div className="app-error-boundary" role="alert">
        <h1>Something went wrong</h1>
        <p>
          Coreside hit an unexpected interface error. Your data is still on this
          device — reload to continue.
        </p>
        <div className="button-row">
          <button type="button" className="btn btn-primary" onClick={this.reload}>
            Reload
          </button>
          <button type="button" className="btn btn-secondary" onClick={this.dismiss}>
            Try again
          </button>
        </div>
      </div>
    );
  }
}
