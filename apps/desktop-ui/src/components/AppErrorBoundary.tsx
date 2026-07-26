import { Component, type ReactNode } from "react";

import { i18n } from "../i18n";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  failed: boolean;
}

/** Last-resort local fallback that prevents a render exception from becoming a blank window. */
export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  public state: AppErrorBoundaryState = { failed: false };

  public static getDerivedStateFromError(): AppErrorBoundaryState {
    return { failed: true };
  }

  public render() {
    if (!this.state.failed) {
      return this.props.children;
    }

    return (
      <main className="flex min-h-screen items-center justify-center bg-surface-base p-8">
        <section
          className="w-full max-w-xl border border-border-default bg-surface-raised p-8 shadow-sm"
          role="alert"
        >
          <h1 className="text-2xl font-semibold text-content-primary">
            {i18n.t("fatalError.title")}
          </h1>
          <p className="mt-3 text-content-secondary">{i18n.t("fatalError.help")}</p>
          <div className="mt-6">
            <button
              type="button"
              className="inline-flex items-center justify-center border border-accent bg-accent px-4 py-2 text-sm font-medium text-on-accent transition-colors hover:bg-accent-strong focus-visible:focus-ring"
              onClick={() => window.location.reload()}
            >
              {i18n.t("fatalError.restart")}
            </button>
          </div>
        </section>
      </main>
    );
  }
}
