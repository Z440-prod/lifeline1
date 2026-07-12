# Lifeline waitlist site

A tiny static page that collects emails into **Firebase Firestore**. No build
step, no server. Emails are stored securely: the security rules let visitors
*add* their email but let **no one read the list from the browser** — you export
it from the Firebase console.

## Files
| File | What it is |
|---|---|
| `index.html` | The waitlist page (on-brand, dark + light, responsive) |
| `app.js` | Form → Firestore logic (validation, dedupe, honeypot) |
| `firebase-config.js` | **You paste your Firebase project config here** |
| `firestore.rules` | Create-only security rules for the `waitlist` collection |
| `firebase.json` | Firebase Hosting + Firestore config |

## Setup (~10 minutes, one time)

1. **Create a Firebase project** at <https://console.firebase.google.com> (free "Spark" plan is enough).
2. **Enable Firestore**: Build → Firestore Database → *Create database* → Production mode → pick a location.
3. **Register a Web app**: Project settings (⚙️) → *Your apps* → Web (`</>`) →
   copy the `firebaseConfig` object → paste the values into **`firebase-config.js`**.
   (These values aren't secret — the security rules protect the data, not the key.)
4. **Install the CLI** (once): `npm install -g firebase-tools`, then `firebase login`.
5. From this `website/` folder: `firebase init` is **not** needed — the config is
   already here. Just link the project and deploy:

   ```bash
   firebase use --add            # pick the project you created
   firebase deploy --only firestore:rules,hosting
   ```

6. Your site is live at `https://<your-project>.web.app`. 🎉

## Seeing the signups
Firebase Console → Firestore Database → the **`waitlist`** collection. Each doc
is one email (id = the address). To export: use the console's export, or the
`gcloud firestore export` command, or the Firebase Admin SDK.

## Notes
- **Dedupe:** each email is its own document, so signing up twice just says
  "already on the list" — no duplicate rows.
- **Spam:** a hidden honeypot field blocks basic bots. For heavier protection add
  Firebase **App Check** (reCAPTCHA) later — no code change to the form needed.
- **Custom domain:** Hosting → Add custom domain, if you want `lifeline.health`
  instead of `*.web.app`.

## Prefer not to use Firebase?
This site can post to the Lifeline backend instead — the Antigravity engine can
expose a `POST /waitlist` route that stores emails in the same Postgres/Supabase
you deploy for the app, so there's nothing extra to run. Ask and it's a small
addition.
