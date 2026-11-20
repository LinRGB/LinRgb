# LinRGB licensing review

MIT is recommended for original LinRGB code and documentation: it is short, permissive and requires preservation of the copyright and permission notices. [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0) is also permissive but adds explicit patent terms (including termination), change notices and NOTICE handling where applicable. [MIT](https://opensource.org/license/mit) has no comparable express patent clause. Neither license grants rights to third-party material.

`cargo metadata --locked` reports MIT, Apache-2.0, Apache-2.0 with LLVM exception, Unlicense alternatives, and Unicode-3.0 conditions across the resolved Rust dependencies. These terms remain applicable; the Cargo `license` field describes original LinRGB code, not its dependency closure. System libraries are under their own terms.

Repository provenance shows references to OpenRGB and SignalRGB protocol research, not a dependency on either application. The OpenRGB-derived PCI catalogue and generated ENE matches are embedded or shipped separately and are not relicensed; retain their provenance and GPL notice. This is a repository review, not proof that every code fragment or asset is original. See [notices](../notices/README.txt) for the exclusions.

Before publishing the licensing change, confirm authority to license the original contributions and to continue distributing the bundled artwork and device descriptor captures. If the branding is to be open-licensed too, record its rights holders and that separate decision first. Existing release tags, native recipes pinned to beta.4 sources and binaries retain their original metadata; review the next native release's aggregate license fields and bundle the current notices when updating its pins.
