// Waitlist form → Firestore. Stores each email as its own document keyed by the
// (lowercased) address, so a second signup is a no-op instead of a duplicate.
// The Firestore rules (firestore.rules) allow create-only and no reads, so the
// list can never be read from the client — only added to.
import { initializeApp } from "https://www.gstatic.com/firebasejs/10.12.0/firebase-app.js";
import {
    getFirestore, doc, setDoc, serverTimestamp,
} from "https://www.gstatic.com/firebasejs/10.12.0/firebase-firestore.js";
import { firebaseConfig } from "./firebase-config.js";

const form = document.getElementById("waitForm");
const emailEl = document.getElementById("email");
const btn = document.getElementById("submitBtn");
const msg = document.getElementById("msg");

const say = (text, kind) => { msg.textContent = text; msg.className = `msg ${kind || ""}`; };
const validEmail = (e) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(e);

// Fail loudly-but-friendly if the config wasn't filled in.
let db = null;
try {
    if (firebaseConfig.apiKey === "REPLACE_ME") throw new Error("unconfigured");
    db = getFirestore(initializeApp(firebaseConfig));
} catch (e) {
    console.error("Firebase not configured — edit firebase-config.js", e);
}

form.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    // Honeypot: real users leave this empty; bots fill it. Silently succeed.
    if (form.company && form.company.value) { say("You're on the list ✓", "ok"); return; }

    const email = emailEl.value.trim().toLowerCase();
    if (!validEmail(email)) { say("Please enter a valid email address.", "err"); return; }
    if (!db) { say("Waitlist isn't configured yet — check back soon.", "err"); return; }

    btn.disabled = true;
    say("Joining…");
    try {
        // Doc id = the email, so re-submitting the same address just no-ops
        // against the create-only rule (caught below as "already on the list").
        await setDoc(doc(db, "waitlist", email), {
            email,
            createdAt: serverTimestamp(),
            source: "web",
        });
        form.reset();
        say("You're on the list ✓ We'll email you when Lifeline opens.", "ok");
    } catch (err) {
        // A duplicate signup hits the create-only rule → permission-denied.
        if (String(err?.code || err).includes("permission-denied")) {
            say("You're already on the list ✓", "ok");
        } else {
            console.error(err);
            say("Something went wrong — please try again.", "err");
        }
    } finally {
        btn.disabled = false;
    }
});
