import * as vscode from "vscode";
import { execFile } from "child_process";

interface PortEntry {
  port: string;
  dir: string;
  role?: string;
}

/** Run `phx-port` in piped mode to get (and auto-register) the port for a folder. */
function getPort(folderPath: string): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile("phx-port", [], { cwd: folderPath }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(stderr.trim() || error.message));
        return;
      }
      const port = stdout.trim();
      if (!/^\d+$/.test(port)) {
        reject(new Error(`Unexpected output from phx-port: ${port}`));
        return;
      }
      resolve(port);
    });
  });
}

/** Parse `phx-port list --flat` to get all registered entries without side effects. */
function listRegistered(): Promise<PortEntry[]> {
  return new Promise((resolve, reject) => {
    execFile("phx-port", ["list", "--flat"], (error, stdout) => {
      if (error) {
        reject(new Error(error.message));
        return;
      }
      // Lines look like: " 4001  /home/user/project" or " 4008  /home/user/project (iframe)"
      const entries: PortEntry[] = [];
      for (const line of stdout.split("\n")) {
        const match = line.match(/^\s*(\d+)\s+(.+?)(?:\s+\((.+)\))?\s*$/);
        if (match) {
          entries.push({ port: match[1], dir: match[2], role: match[3] });
        }
      }
      resolve(entries);
    });
  });
}

/**
 * Given a file path, find the registered project whose directory is the
 * longest prefix of that path. Returns the main-role port entry, or undefined.
 */
function findProjectForFile(
  filePath: string,
  entries: PortEntry[]
): PortEntry | undefined {
  const mainEntries = entries.filter((e) => !e.role);
  // Sort longest path first so the first match is the most specific
  mainEntries.sort((a, b) => b.dir.length - a.dir.length);
  return mainEntries.find(
    (e) => filePath === e.dir || filePath.startsWith(e.dir + "/")
  );
}

function resolveFolder(uri?: vscode.Uri): string | undefined {
  if (uri) {
    return uri.fsPath;
  }
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (workspaceFolders && workspaceFolders.length === 1) {
    return workspaceFolders[0].uri.fsPath;
  }
  return undefined;
}

export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    // Right-click folder → open in browser (auto-registers if needed)
    vscode.commands.registerCommand(
      "phx-port.openInBrowser",
      async (uri?: vscode.Uri) => {
        const folder = resolveFolder(uri);
        if (!folder) {
          vscode.window.showErrorMessage(
            "phx-port: Right-click a folder in the Explorer, or open a single-folder workspace."
          );
          return;
        }
        try {
          const port = await getPort(folder);
          const url = vscode.Uri.parse(`http://localhost:${port}`);
          await vscode.env.openExternal(url);
        } catch (e: unknown) {
          const msg = e instanceof Error ? e.message : String(e);
          vscode.window.showErrorMessage(`phx-port: ${msg}`);
        }
      }
    ),

    // Right-click folder → show port
    vscode.commands.registerCommand(
      "phx-port.showPort",
      async (uri?: vscode.Uri) => {
        const folder = resolveFolder(uri);
        if (!folder) {
          vscode.window.showErrorMessage(
            "phx-port: Right-click a folder in the Explorer, or open a single-folder workspace."
          );
          return;
        }
        try {
          const port = await getPort(folder);
          const folderName = folder.split("/").pop() || folder;
          await vscode.window.showInformationMessage(
            `${folderName} → http://localhost:${port}`
          );
        } catch (e: unknown) {
          const msg = e instanceof Error ? e.message : String(e);
          vscode.window.showErrorMessage(`phx-port: ${msg}`);
        }
      }
    ),

    // Keybinding: open browser for the project that the active file belongs to
    vscode.commands.registerCommand(
      "phx-port.openFromActiveFile",
      async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
          vscode.window.showErrorMessage("phx-port: No active editor.");
          return;
        }
        const filePath = editor.document.uri.fsPath;
        try {
          const entries = await listRegistered();
          const match = findProjectForFile(filePath, entries);
          if (!match) {
            vscode.window.showWarningMessage(
              "phx-port: This file is not inside a registered project. Use the Explorer context menu to register a folder first."
            );
            return;
          }
          const url = vscode.Uri.parse(`http://localhost:${match.port}`);
          await vscode.env.openExternal(url);
        } catch (e: unknown) {
          const msg = e instanceof Error ? e.message : String(e);
          vscode.window.showErrorMessage(`phx-port: ${msg}`);
        }
      }
    )
  );
}

export function deactivate() {}
