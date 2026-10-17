# 09 — Build macOS and publish an iOS app

There are separate Apple distribution paths. A Mac app does not become an iPhone app simply by uploading its DMG to App Store Connect. Tauri shares much of the UI/Rust code, but target-specific authentication, lifecycle and signing must be completed.

The supplied macOS shell uses a separate local or Azure backend. The iOS shell scaffold is included, but its `login` command intentionally returns an explicit unfinished-integration error. Do not submit that version as a finished app. This chapter gives the remaining integration and publishing steps. Apple builds/signing, your tenant login and store approval have not been performed in this environment.

## 1. Choose your real identity

1. Enroll in the [Apple Developer Program](https://developer.apple.com/programs/) if distributing through an App Store or signing for wider distribution. Confirm current membership and eligibility requirements directly with Apple.
2. Choose a unique reverse-DNS identifier, for example `com.yourcompany.gentsoffice`. Replace `com.example.gentsoffice` in `src-tauri/tauri.conf.json` before generating Apple projects.
3. Set a release version and keep a monotonically increasing Apple build number for uploads.
4. Create a 1024×1024 source icon you own. Generate the platform icon assets:

```bash
npm run tauri icon -- /absolute/path/to/your-icon.png
```

Check the generated Tauri configuration and Apple asset catalogs; the starter's icon list is intentionally empty. Inspect icons at small sizes.

## 2. Build the local Mac application

With Xcode selected and prerequisites installed:

```bash
rustup target add aarch64-apple-darwin
npm ci
npm run tauri build -- --target aarch64-apple-darwin
```

The ARM build artifacts appear under `src-tauri/target/aarch64-apple-darwin/release/bundle/`. Open the `.app`, connect to the backend and complete a task. A local backend must already be running for `127.0.0.1`; an Azure backend makes the app independently usable without a local server.

For public direct distribution, configure the **Developer ID Application** certificate, hardened runtime and notarization using Tauri's macOS signing guide. Store signing credentials in Keychain/CI secrets. Use Apple's notarization workflow and staple the ticket. Test the downloaded DMG on a clean Mac; launching an app from your development folder is not a distribution test.

## 3. Mac App Store path

Use the Mac App Store rather than the direct-download path if that is your goal:

1. Create a macOS app record in App Store Connect and register its explicit bundle ID.
2. Configure Apple Distribution and Mac Installer Distribution certificates/provisioning as required by the current Tauri/Apple guide.
3. Configure App Sandbox and only required entitlements, including outbound networking. The current desktop loopback sign-in also needs a listening socket; replace it with the native MSAL custom-scheme integration or validate and entitle it before submission.
4. Build the app bundle using the App Store configuration described in the Tauri guide, package a signed installer and upload through the current supported Apple tooling.
5. Test through the macOS TestFlight path before review.

Do not reuse a Developer ID distribution configuration as the Mac App Store signing configuration.

## 4. Generate the iOS project

Select full Xcode, accept its license, and install an iOS simulator runtime in Xcode settings:

```bash
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
npm run tauri ios init
npm run tauri ios dev
```

Follow the CLI prompts to select your developer team and simulator. The generated Xcode project lives under `src-tauri/gen/apple`. Connect the iOS UI to your Azure HTTPS backend: `127.0.0.1` on a physical phone means the phone, not your Mac.

Confirm office layout, scrolling, buttons, keyboard and safe areas on a small phone. Disable unnecessary rendering while the app is backgrounded. Long tasks execute in Azure; mobile suspension must not be treated as task cancellation.

## 5. Finish native iOS Entra sign-in

Use Microsoft's supported iOS MSAL library through a Tauri mobile plugin. Rust still owns orchestration; a small Swift bridge handles the platform authentication session and Keychain. This is necessary native integration work; the desktop loopback command is not the iOS implementation.

1. Under the **same native client registration** from chapter 05, add an **iOS/macOS** platform with your final bundle ID. Copy the generated `msauth.<bundle-id>://auth` redirect URI. Keeping the same client ID lets the API's existing `azp` allowlist accept it. If you create a separate iOS client, extend the API's client allowlist explicitly.
2. Follow Microsoft's linked Swift preparation tutorial to add MSAL to the generated Xcode app through Swift Package Manager or its supported package integration. Configure the URL scheme, query schemes and Keychain Sharing as required. Pin the dependency version in the Apple project.
3. Create a Tauri mobile plugin using the installed CLI's **plugin init** workflow, with an iOS Swift package. Read the generated `mobile.rs`, command permission files and Swift example. Use `npm run tauri plugin -- --help` to inspect the pinned CLI's workflow and options; do not modify Tauri's internal generated runtime sources.
4. Add the MSAL dependency to that plugin's Swift package or use a native wrapper target linked into the app. Expose one `login` command returning only an API access token to the app. Run UI calls on the main thread and retain the MSAL application while acquisition is active.
5. Implement interactive acquisition with your tenant authority, native client ID, registered redirect URI and scope `api://<API_CLIENT_ID>/Harness.Run`. The core Swift call follows this shape (within the native controller/plugin):

```swift
import UIKit
import MSAL

func acquireHarnessToken(
    tenantId: String,
    clientId: String,
    redirectUri: String,
    apiId: String,
    presenter: UIViewController,
    completion: @escaping (Result<String, Error>) -> Void
) throws -> MSALPublicClientApplication {
    let authority = try MSALAADAuthority(
        url: URL(string: "https://login.microsoftonline.com/\(tenantId)")!
    )
    let configuration = MSALPublicClientApplicationConfig(
        clientId: clientId,
        redirectUri: redirectUri,
        authority: authority
    )
    let application = try MSALPublicClientApplication(configuration: configuration)
    let web = MSALWebviewParameters(authPresentationViewController: presenter)
    let parameters = MSALInteractiveTokenParameters(
        scopes: ["api://\(apiId)/Harness.Run"],
        webviewParameters: web
    )
    application.acquireToken(with: parameters) { result, error in
        if let error = error { completion(.failure(error)); return }
        if let result = result { completion(.success(result.accessToken)) }
    }
    return application // retain this in your plugin/controller
}
```

This is the acquisition core, not a complete drop-in Tauri plugin. Use Tauri's native `Invoke` argument/resolve/reject pattern and generated plugin registration to bridge it. Return a clear rejection for cancellation or missing result. Forward authentication callback URLs to MSAL exactly as its tutorial requires; do not write your own token parser or embedded sign-in web form.

6. Register the plugin in the Tauri builder and allow only its required commands in the main window capability. Update the iOS frontend path to invoke that plugin rather than the explicit `login` stub, or have the Rust command delegate to its mobile handle. Keep desktop behavior under its existing platform condition.
7. Implement silent token acquisition using MSAL's supported account cache, fallback to interactive acquisition, and proper account removal/sign-out UX. Keep refresh tokens in the platform library's secure storage; do not expose them to JavaScript or write them into localStorage.
8. Replace the development token-entry form with a production sign-in screen. Bundle nonsecret tenant/client/API configuration and the fixed HTTPS API origin. Remove arbitrary backend addresses from the release UI, and narrow Tauri's `connect-src` from general HTTPS to that API domain.
9. Test login, MFA, cancellation, expired tokens, reinstall, different users, offline behavior and returning from background. Re-run the API's denied-access checks from chapter 05.

**Release gate:** do not proceed to public review until the native bridge and these tests pass on an actual device. The starter labels this unfinished portion rather than pretending desktop auth also works on iOS.

## 6. Make runs resumable before mobile release

The final starter now includes the SQLite journal and reconnect UI described in chapter 13. Configure durable cloud storage and validate restart behavior before mobile release. Persist the last run ID in app storage (not credentials), reload it after sign-in and resume polling. Verify the app can be suspended mid-task and later display the completed result. The cloud worker should keep running across client disconnections; backend redeploys require durable checkpoints.

## 7. Sign and test iOS

1. In Xcode, set the correct team, final bundle ID, version and build number. Use automatic signing initially if appropriate for your team.
2. Select a physical iPhone. Fix provisioning/entitlement errors rather than disabling protections.
3. Test sign-in and the full live-provider task on the device with Wi-Fi and cellular.
4. Verify TLS, consent, readable event logs and realistic missing-usage handling. Remove simulator-only API addresses and debug token entry.
5. Create an iOS app record in App Store Connect for the exact bundle ID.
6. Set the development team in your Tauri Apple configuration as required by the current signing guide. Build for App Store Connect:

```bash
npm run tauri ios build -- --export-method app-store-connect
```

The current Tauri guide places the generated IPA under `src-tauri/gen/apple/build/arm64/`; use the actual product filename from the build output. Alternatively open the generated project in Xcode, select an archive destination, **Product → Archive**, then use **Organizer → Distribute App → App Store Connect → Upload**. Follow the supported archive workflow for the installed Tauri version.

## 8. TestFlight and App Store review

1. Wait for build processing and complete export-compliance questions accurately.
2. Add internal TestFlight testers, then external testers if needed; external distribution may require beta review.
3. Provide app description, support URL, privacy policy, screenshots for supported device classes, age rating and reviewer instructions.
4. Disclose actual task-data handling, providers, retention and telemetry in App Privacy. Obtain explicit permission where Apple's current third-party AI/personal-data sharing rules require it. The lab consent checkbox is a starting mechanism, not a complete release privacy experience.
5. If Microsoft sign-in is the only sign-in option, assess the current App Review Guideline 4.8 conditions and enterprise/business exceptions. Do not assume an exception applies to a public consumer product. Implement any required alternative login without weakening API authorization.
6. If you sell digital subscriptions or model credits, evaluate current storefront-specific purchase rules and implement appropriate StoreKit flows where required. The starter includes no billing system.
7. Give reviewers a working test account or approved review access for the app's intended audience and a reliable running backend. Explain how to start a safe demo task. An inaccessible internal tenant can prevent review.
8. Submit the chosen build for review. Address Apple's actual feedback and retest. Approval is Apple's decision, not guaranteed by successful compilation.
9. Release manually or with the chosen staged release setting after approval. Monitor backend capacity, error rates and spending.

**Checkpoint:** a physical iPhone runs native sign-in and a resumable task using your Azure API; TestFlight works before public submission. A `.dmg` is for Mac distribution, an `.ipa` is for iOS distribution.
