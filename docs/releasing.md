# Native releases

1. Set `package.version` in `Cargo.toml` and update `Cargo.lock` to match. Commit the change on `main` through the usual review process. Wait for CI on that commit. Do not tag a version that already has a tag, release or draft.
2. In **Actions → Native release packages → Run workflow**, select `main`. This is a dry run: it builds and installs all three packages, tests them and uploads `native-release-bundle` to the run. It never creates or edits a GitHub Release. Inspect its `SOURCE-COMMIT`, native package versions and `SHA256SUMS`.
3. When ready, push an annotated tag on the **exact** SHA you tested. For example:

   ```bash
   sha='replace-with-the-verified-full-commit-sha'
   git fetch origin main --tags &&
     git merge-base --is-ancestor "$sha" origin/main &&
     git tag -a v1.0.0-beta.5 "$sha" -m 'LinRGB v1.0.0-beta.5' &&
     git push origin refs/tags/v1.0.0-beta.5
   ```

   The tag push is the explicit publication trigger. Do not create the release via the GitHub Releases UI first. Tags must match Cargo exactly: `vMAJOR.MINOR.PATCH`, optionally with `-alpha.N`, `-beta.N` or `-rc.N`.
4. Wait for all builds and installation checks. The workflow creates a draft, uploads the source archive, three native packages, `SOURCE-COMMIT` and `SHA256SUMS`, checks all six uploads against their local digests, and only then publishes. Prerelease versions are marked as such. Download the assets and run `sha256sum --check SHA256SUMS`.

The build jobs use pinned official distro images and the existing Arch, Debian and Fedora recipes. The archive is produced once from the tag tree by `tools/release-source.sh`; each builder checks its digest. Package repositories are not frozen by the image pin. The diagnostic smoke test runs as an ordinary user in a disposable container: it is not a physical RGB test.

If a build fails, fix it before publishing. Rerun the failed tag workflow for a transient failure; if the tagged source itself needs a fix, use a new version and tag. Do not move a tag. If upload fails, its incomplete draft stays private. Review and remove **only that incomplete unpublished draft** before rerunning the failed publication job. Existing drafts and published releases are never overwritten. Manual dispatch remains build-only, even if launched on a tag.

For historical releases, use the native recipes from their corresponding tags. The standalone recipe defaults remain pinned to the beta.4 archive; the release workflow passes the current version and archive checksum explicitly.
