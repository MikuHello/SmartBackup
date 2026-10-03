# Third-party components

- Project-local skills: [mattpocock/skills](https://github.com/mattpocock/skills), revision `d81f3a183412e71a5b1e84ca21bc1a35eea03a60`, MIT. Original license retained at `.agents/skills/LICENSE-mattpocock`. No automatic upstream updates.
- 7-Zip is a separately installed external executable in this prototype; not redistributed in this repository. Official releases and licenses: https://www.7-zip.org/download.html and https://www.7-zip.org/license.txt. The application pins the executable hash after explicit configuration; that pin detects changes, it does not authenticate the publisher.
- Rust dependency versions and checksums are locked in Cargo.lock. Release-grade SBOM, dependency license inventory and signed packages remain release work, not completed by this prototype.
