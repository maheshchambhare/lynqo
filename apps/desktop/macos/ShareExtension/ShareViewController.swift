import Cocoa
import Social

class ShareViewController: NSViewController {

    override func viewDidLoad() {
        super.viewDidLoad()
        
        guard let extensionContext = self.extensionContext,
              let inputItems = extensionContext.inputItems as? [NSExtensionItem] else {
            cancel()
            return
        }
        
        Task {
            await processItems(inputItems)
            complete()
        }
    }
    
    private func processItems(_ items: [NSExtensionItem]) async {
        for item in items {
            guard let attachments = item.attachments else { continue }
            for provider in attachments {
                if provider.hasItemConformingToTypeIdentifier("public.file-url") {
                    do {
                        let itemData = try await provider.loadItem(forTypeIdentifier: "public.file-url", options: nil)
                        if let url = itemData as? URL {
                            await shareFile(path: url.path)
                        }
                    } catch {
                        NSLog("lynqo Share Extension error loading file URL: \(error)")
                    }
                }
                else if provider.hasItemConformingToTypeIdentifier("public.plain-text") {
                    do {
                        let itemData = try await provider.loadItem(forTypeIdentifier: "public.plain-text", options: nil)
                        if let text = itemData as? String {
                            await shareText(text: text)
                        }
                    } catch {
                        NSLog("lynqo Share Extension error loading plain text: \(error)")
                    }
                }
                else if provider.hasItemConformingToTypeIdentifier("public.url") {
                    do {
                        let itemData = try await provider.loadItem(forTypeIdentifier: "public.url", options: nil)
                        if let url = itemData as? URL {
                            await shareText(text: url.absoluteString)
                        }
                    } catch {
                        NSLog("lynqo Share Extension error loading URL: \(error)")
                    }
                }
            }
        }
    }
    
    private func shareFile(path: String) async {
        guard let url = URL(string: "http://127.0.0.1:7432/api/files/share") else { return }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        
        let body: [String: String] = ["path": path]
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            if let httpResponse = response as? HTTPURLResponse {
                NSLog("lynqo Share Extension: share file response status code \(httpResponse.statusCode)")
            }
        } catch {
            NSLog("lynqo Share Extension: failed to share file: \(error)")
        }
    }
    
    private func shareText(text: String) async {
        guard let url = URL(string: "http://127.0.0.1:7432/api/clipboard") else { return }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        
        let body: [String: String] = ["text": text]
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            if let httpResponse = response as? HTTPURLResponse {
                NSLog("lynqo Share Extension: share text response status code \(httpResponse.statusCode)")
            }
        } catch {
            NSLog("lynqo Share Extension: failed to share text: \(error)")
        }
    }
    
    private func complete() {
        self.extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
    
    private func cancel() {
        self.extensionContext?.cancelRequest(withError: NSError(domain: "lynqo", code: -1, userInfo: nil))
    }
}
