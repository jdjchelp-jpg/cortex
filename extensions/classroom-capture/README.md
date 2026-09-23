# Cortex Classroom Capture

Load this folder as an unpacked extension in Chrome (`chrome://extensions` → Developer mode → Load unpacked) or Firefox (`about:debugging` → This Firefox → Load Temporary Add-on → `manifest.json`).

Open a Google Classroom page, click the extension, enter a Cortex subject and topic/subtopic such as `Biology / Cell structure`, then choose **Capture visible links/files**. The extension creates a `.cortex.json` bundle without sending data to a third-party server.

This first version exports visible links and file links from the current page. The Cortex native import bridge will be added next so the bundle can be inserted automatically into the selected subject/topic.
