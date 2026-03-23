# Product Requirements Document (PRD): Spring Boot TUI (tsb)

## 1. Overview
`tsb` (Terminal Spring Boot) is a Terminal User Interface (TUI) application inspired by `k9s`, designed specifically for Spring Boot developers. It aims to streamline the process of bootstrapping new Spring Boot applications and managing/monitoring running Spring Boot instances via their Actuator endpoints directly from the terminal.

## 2. Core Features

### 2.1. Project Generation (Spring Initializr Integration)
*   **Interactive Generation:** A TUI wizard allowing users to select Spring Boot project parameters (Language, Build System, Spring Boot Version, Project Metadata, Dependencies) similar to start.spring.io.
*   **API Integration:** Fetch metadata and options from the official Spring Initializr API (`https://start.spring.io/`).
*   **Caching:** Cache the Initializr metadata (dependencies, versions, etc.) locally for a configurable duration (e.g., 24 hours) to improve responsiveness and reduce API calls.
*   **Download & Extract:** Generate the project archive and extract it to the desired local directory.

### 2.2. Application Management & Monitoring
*   **Manual Addition:** Allow users to manually add running Spring Boot applications by providing their base URL/port (assuming Actuator is enabled).
*   **Auto-Discovery:** Automatically scan common local ports (e.g., 8080, 8081, etc.) or use local process discovery to find running Spring Boot applications with accessible Actuator endpoints.
*   **Actuator Integration:** Act as a client for standard Spring Boot Actuator endpoints.

### 2.3. Resource Views (k9s-style)
The TUI will offer distinct views for different "resources" exposed by the Actuator:
*   **Applications View:** The main dashboard listing all discovered/added Spring Boot apps and their basic health status.
*   **Loggers View:** View current log levels for different packages/classes. **Action:** Interactively change log levels at runtime.
*   **Thread Dump View:** Trigger and view a thread dump of the running JVM.
*   **Heap Dump View:** Trigger a heap dump (potentially saving to a local file or providing a download link).
*   **Beans View:** Browse the Spring application context to see all loaded beans and their dependencies.
*   **Endpoints / Mappings View:** View all exposed HTTP endpoints and their mappings.
*   **Environment / Config View:** (Optional but recommended) View environment variables and configuration properties.

## 3. Technical Stack
*   **Language:** Go
*   **TUI Framework:** `bubbletea` (Charmbracelet ecosystem)
    *   `bubbles` for common components (lists, text inputs, spinners).
    *   `lipgloss` for styling.
*   **HTTP Client:** Go standard `net/http` for interacting with Spring Initializr and Actuator endpoints.
*   **Storage/Cache:** Local file-based storage (e.g., JSON or SQLite) for caching Initializr metadata and saving configured applications.

## 4. User Interface (TUI) Flow
1.  **Start `tsb`:**
    *   If no apps are configured/found, prompt the user with a main menu: `[Generate New Project] | [Add App] | [Scan Local Apps]`.
    *   If apps are found, open the **Applications View** (default dashboard).
2.  **Navigation:** Use keyboard shortcuts (Vim-style `j`/`k`, arrows, Enter, Esc) to navigate lists and views. Use specific keys (e.g., `:` to open a command palette like k9s) to switch between resource types (e.g., typing `:beans` switches to the Beans view for the selected app).
3.  **Actions:** Provide context-sensitive actions at the bottom of the screen (e.g., `[L] Change Log Level`, `[D] Thread Dump`).

## 5. Implementation Phases
*   **Phase 1: Project Generation:** Implement the Spring Initializr API integration, caching, and the TUI wizard to generate and download projects.
*   **Phase 2: Discovery & Dashboard:** Implement manual app addition, basic local port scanning, and the main Applications view showing health status.
*   **Phase 3: Core Actuator Views:** Implement Loggers (with update capability), Beans, and Mappings views.
*   **Phase 4: Advanced Actuator Views:** Implement Thread Dump and Heap Dump interactions.

## 6. Resolved Decisions & Additional Constraints
*   **Persistence:** The list of manually added applications and any application configuration will be persisted in `~/.config/tsb/config.yml`.
*   **Heap Dumps:** Heap dumps will be fully downloaded to the local machine. This sets up the foundation for future features, such as integrating flamegraph visualizations or advanced analysis.
*   **UI/UX:** The TUI will closely mimic `k9s` shortcuts and interaction models (e.g., `/` for search, `:` for the command palette, Vim-style navigation) to provide a familiar and efficient experience.
*   **Dependencies:** All third-party libraries and tools used in this project must explicitly use the latest available up-to-date versions. Always verify the latest version before adding a dependency.
