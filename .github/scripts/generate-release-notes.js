/**
 * GitHub Action script to generate release notes with smart changelog logic
 * This script handles both prerelease and stable release note generation
 */

const fs = require('fs');

/**
 * Parse a semantic version tag into components
 * @param {string} tag - The version tag (e.g., "v1.2.3-beta.1" or "1.2.3")
 * @returns {Object|null} - Parsed version object or null if invalid
 */
function parseVersion(tag) {
  const match = tag.match(/^v?(\d+)\.(\d+)\.(\d+)(?:-(.+))?$/);
  if (!match) return null;
  
  return {
    major: parseInt(match[1]),
    minor: parseInt(match[2]),
    patch: parseInt(match[3]),
    prerelease: match[4] || null,
    original: tag
  };
}

/**
 * Compare two parsed version objects for sorting (newest first)
 * @param {Object} a - First version object
 * @param {Object} b - Second version object
 * @returns {number} - Comparison result
 */
function compareVersions(a, b) {
  if (a.major !== b.major) return b.major - a.major;
  if (a.minor !== b.minor) return b.minor - a.minor;
  if (a.patch !== b.patch) return b.patch - a.patch;
  
  // Handle prerelease comparison
  if (a.prerelease && !b.prerelease) return 1; // stable > prerelease
  if (!a.prerelease && b.prerelease) return -1; // stable > prerelease
  if (a.prerelease && b.prerelease) {
    return b.prerelease.localeCompare(a.prerelease);
  }
  return 0;
}

/**
 * Find the appropriate previous tag for changelog generation
 * @param {Array} tags - Array of tag objects from GitHub API
 * @param {string} currentTag - The current release tag
 * @param {boolean} isPrerelease - Whether the current release is a prerelease
 * @returns {string} - The previous tag name or empty string
 */
function findPreviousTag(tags, currentTag, isPrerelease) {
  console.log('Current tag:', currentTag, 'Is prerelease:', isPrerelease);
  console.log('Available tags:', tags.map(t => t.name).slice(0, 10));
  
  // Parse and sort all valid tags by semantic version
  const validTags = tags
    .map(tag => parseVersion(tag.name))
    .filter(parsed => parsed !== null)
    .sort(compareVersions);
  
  console.log('Parsed and sorted tags:', validTags.map(v => v.original).slice(0, 10));
  
  // Find the current tag index
  const currentIndex = validTags.findIndex(v => v.original === currentTag);
  if (currentIndex === -1) {
    console.log('Current tag not found in parsed tags, using original tag list approach');
    // Fallback to simple approach
    const tagIndex = tags.findIndex(tag => tag.name === currentTag);
    if (tagIndex > 0) {
      const fallbackTag = tags[tagIndex + 1].name;
      console.log('Fallback: using immediate previous tag:', fallbackTag);
      return fallbackTag;
    }
    return '';
  }
  if (isPrerelease) {
    console.log('Processing prerelease - looking for previous prerelease or last stable');
    // For prereleases: find the previous prerelease in the SAME version line OR the last stable release
    
    const currentVersion = parseVersion(currentTag);
    if (!currentVersion) {
      console.log('Could not parse current tag, using fallback');
      return '';
    }
    
    // First try to find the previous prerelease in the same major.minor.patch version
    const previousPrerelease = validTags
      .slice(currentIndex + 1)
      .find(v => 
        v.prerelease !== null && 
        v.major === currentVersion.major && 
        v.minor === currentVersion.minor && 
        v.patch === currentVersion.patch
      );
    
    if (previousPrerelease) {
      console.log('Found previous prerelease in same version:', previousPrerelease.original);
      return previousPrerelease.original;
    }
    
    // If no previous prerelease in same version, find the last stable release
    const lastStable = validTags
      .slice(currentIndex + 1)
      .find(v => v.prerelease === null);
    
    if (lastStable) {
      console.log('No previous prerelease in same version, using last stable:', lastStable.original);
      return lastStable.original;
    }
    
    console.log('No previous releases found, showing all changes');
    return '';
  } else {
    console.log('Processing stable release - looking for previous stable release');
    // For stable releases: find the previous stable release (skip all prereleases)
    const previousStable = validTags
      .slice(currentIndex + 1)
      .find(v => v.prerelease === null);
    
    if (previousStable) {
      console.log('Found previous stable release:', previousStable.original);
      return previousStable.original;
    }
    
    console.log('No previous stable release found, showing all changes');
    return '';
  }
}

/**
 * Generate release notes using GitHub's API and template processing
 * @param {Object} github - GitHub API client
 * @param {Object} context - GitHub action context
 * @returns {Promise<void>}
 */
async function generateReleaseNotes(github, context) {
  try {
    // Get the current release
    const { data: release } = await github.rest.repos.getRelease({
      owner: context.repo.owner,
      repo: context.repo.repo,
      release_id: context.payload.release.id
    });

    const currentTag = release.tag_name;
    const isPrerelease = release.prerelease;

    // Read release notes template
    let template = '';
    try {
      template = fs.readFileSync('.github/release-notes-template.md', 'utf8');
      console.log('Successfully loaded release notes template');
    } catch (error) {
      console.log('Could not read release notes template:', error.message);
      template = '# Release Notes\n\n<!-- CHANGELOG_INSERTION_POINT -->\n\n## Downloads\n\nTemplate not found.';
    }

    // Smart tag selection for proper changelog range
    let previousTag = '';
    try {
      const { data: tags } = await github.rest.repos.listTags({
        owner: context.repo.owner,
        repo: context.repo.repo,
        per_page: 100
      });

      previousTag = findPreviousTag(tags, currentTag, isPrerelease);
    } catch (error) {
      console.log('Could not get previous tag:', error.message);
    }

    // Generate GitHub's native release notes with proper range
    let releaseNotesParams = {
      owner: context.repo.owner,
      repo: context.repo.repo,
      tag_name: currentTag,
      target_commitish: release.target_commitish || 'main'
    };

    if (previousTag) {
      releaseNotesParams.previous_tag_name = previousTag;
      console.log('Changelog range:', previousTag, '->', currentTag);
    } else {
      console.log('Generating changelog from beginning of history');
    }

    let githubNotes = '';
    try {
      const { data: generatedNotes } = await github.rest.repos.generateReleaseNotes(releaseNotesParams);
      githubNotes = generatedNotes.body;
      console.log('Successfully generated GitHub changelog');
    } catch (error) {
      console.log('Could not generate GitHub release notes:', error.message);
      githubNotes = '## What\'s Changed\n\nChangelog generation failed. Please try again later.';
    }

    // Process the template
    let releaseBody = template;

    // Handle beta warning section
    if (release.prerelease) {
      // Keep the beta warning in the template as-is
      console.log('Beta release detected - keeping beta warning');
    } else {
      // Remove the beta warning section for stable releases
      releaseBody = releaseBody.replace(/## Header for Beta Releases[\s\S]*?---\s*/g, '');
      console.log('Stable release - removed beta warning section');
    }

    // Replace placeholders (handle both formats)
    releaseBody = releaseBody.replace(/\{VERSION\}/g, release.tag_name);
    releaseBody = releaseBody.replace(/\\{VERSION\\}/g, release.tag_name);

    // Insert the changelog at the designated insertion point
    const insertionPoint = '<!-- CHANGELOG_INSERTION_POINT -->';
    const insertionIndex = releaseBody.indexOf(insertionPoint);
    if (insertionIndex !== -1) {
      releaseBody = releaseBody.slice(0, insertionIndex) + 
                  githubNotes + '\n\n' +
                  releaseBody.slice(insertionIndex + insertionPoint.length);
      console.log('Changelog inserted at designated insertion point');
    } else {
      // Fallback: look for Downloads section (backward compatibility)
      const downloadsSectionIndex = releaseBody.indexOf('## Downloads');
      if (downloadsSectionIndex !== -1) {
        releaseBody = releaseBody.slice(0, downloadsSectionIndex) + 
                    githubNotes + '\n\n' +
                    releaseBody.slice(downloadsSectionIndex);
        console.log('Changelog inserted before Downloads section (fallback)');
      } else {
        // Last resort: just append changelog
        releaseBody = releaseBody + '\n\n' + githubNotes;
        console.log('Changelog appended to end (no insertion point found)');
      }
    }

    console.log('Final release body length:', releaseBody.length);

    // Update the release with generated notes
    await github.rest.repos.updateRelease({
      owner: context.repo.owner,
      repo: context.repo.repo,
      release_id: context.payload.release.id,
      body: releaseBody
    });

    console.log('Release notes updated successfully!');
  } catch (error) {
    console.error('Error generating release notes:', error);
    throw error;
  }
}

// Export for use in GitHub Actions
module.exports = { generateReleaseNotes, parseVersion, compareVersions, findPreviousTag };
