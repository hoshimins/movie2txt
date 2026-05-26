use serde::{Deserialize, Serialize};

pub const MIN_SUBTITLE_DURATION_MS: i64 = 600;
pub const MAX_SUBTITLE_DURATION_MS: i64 = 3_000;
const MAX_HEAL_GAP_MS: i64 = 250;

const SPLIT_EXPRESSIONS: &[&str] = &[
    "ですね",
    "じゃん",
    "だよ",
    "けど",
    "から",
    "ので",
    "って",
    "ます",
    "です",
    "ね",
    "よ",
    "な",
];

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SubtitleEntry {
    pub index: usize,
    pub start_time: String,
    pub end_time: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubtitleChunk {
    raw_text: String,
    text: String,
}

pub fn apply_character_limit_to_content(content: &str, max_chars: usize) -> Result<String, String> {
    let entries = parse_srt(content)?;
    let entries = apply_character_limit_to_entries(entries, max_chars)?;
    Ok(render_srt(&entries))
}

pub fn apply_character_limit_to_entries(
    entries: Vec<SubtitleEntry>,
    max_chars: usize,
) -> Result<Vec<SubtitleEntry>, String> {
    if max_chars == 0 {
        return Ok(entries);
    }

    let mut new_entries = Vec::new();
    for entry in entries {
        new_entries.extend(build_split_entries(entry, max_chars)?);
    }

    new_entries = heal_split_artifacts(new_entries, max_chars)?;
    reindex_entries(&mut new_entries);
    Ok(new_entries)
}

pub fn parse_srt(content: &str) -> Result<Vec<SubtitleEntry>, String> {
    let mut entries = Vec::new();
    let content = content.replace("\r\n", "\n");
    let blocks: Vec<&str> = content
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .collect();

    for block in blocks {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 {
            continue;
        }

        let index = lines[0]
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("インデックスのパースに失敗しました: {}", lines[0]))?;

        let time_parts: Vec<&str> = lines[1].split(" --> ").collect();
        if time_parts.len() != 2 {
            continue;
        }

        entries.push(SubtitleEntry {
            index,
            start_time: time_parts[0].trim().to_string(),
            end_time: time_parts[1].trim().to_string(),
            text: lines[2..].join("\n"),
        });
    }

    Ok(entries)
}

pub fn render_srt(entries: &[SubtitleEntry]) -> String {
    let mut content = String::new();

    for entry in entries {
        content.push_str(&entry.index.to_string());
        content.push('\n');
        content.push_str(&entry.start_time);
        content.push_str(" --> ");
        content.push_str(&entry.end_time);
        content.push('\n');
        content.push_str(&entry.text);
        content.push_str("\n\n");
    }

    content
}

fn reindex_entries(entries: &mut [SubtitleEntry]) {
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.index = index + 1;
    }
}

fn normalize_subtitle_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\r' | '\n' | '\t' => ' ',
            _ => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_subtitle_text(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '。' | '、' | '！' | '？' | '!' | '?' | '…' | '・'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_split_punctuation(c: char) -> bool {
    matches!(c, '。' | '、' | '！' | '？' | '!' | '?' | '…' | '・')
}

fn div_ceil_usize(value: usize, divisor: usize) -> usize {
    (value + divisor - 1) / divisor
}

fn div_ceil_i64(value: i64, divisor: i64) -> i64 {
    (value + divisor - 1) / divisor
}

fn chars_match(chars: &[char], start: usize, needle: &[char]) -> bool {
    start + needle.len() <= chars.len() && chars[start..start + needle.len()] == needle[..]
}

fn find_punctuation_split(chars: &[char], start: usize, window_end: usize) -> Option<usize> {
    (start..window_end)
        .rev()
        .find(|&index| is_split_punctuation(chars[index]))
        .map(|index| index + 1)
}

fn find_whitespace_split(chars: &[char], start: usize, window_end: usize) -> Option<usize> {
    (start..window_end)
        .rev()
        .find(|&index| chars[index].is_whitespace())
        .map(|index| index + 1)
}

fn find_expression_split(chars: &[char], start: usize, window_end: usize) -> Option<usize> {
    for end in (start + 1..=window_end).rev() {
        for expression in SPLIT_EXPRESSIONS {
            let needle: Vec<char> = expression.chars().collect();
            if needle.len() > end - start {
                continue;
            }

            let candidate_start = end - needle.len();
            if chars_match(chars, candidate_start, &needle) {
                return Some(end);
            }
        }
    }

    None
}

fn choose_split_index(chars: &[char], start: usize, max_chars: usize) -> usize {
    let window_end = (start + max_chars).min(chars.len());

    find_punctuation_split(chars, start, window_end)
        .or_else(|| find_whitespace_split(chars, start, window_end))
        .or_else(|| find_expression_split(chars, start, window_end))
        .unwrap_or(window_end.max(start + 1))
}

fn split_subtitle_text(text: &str, max_chars: usize) -> Vec<SubtitleChunk> {
    let normalized = normalize_subtitle_text(text);
    if normalized.is_empty() {
        return Vec::new();
    }

    let sanitized = sanitize_subtitle_text(&normalized);
    if sanitized.is_empty() {
        return Vec::new();
    }

    if max_chars == 0 || sanitized.chars().count() <= max_chars {
        return vec![SubtitleChunk {
            raw_text: normalized,
            text: sanitized,
        }];
    }

    let chars: Vec<char> = normalized.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = if chars.len() - start <= max_chars {
            chars.len()
        } else {
            choose_split_index(&chars, start, max_chars)
        };

        let raw_text = chars[start..end].iter().collect::<String>();
        if !raw_text.trim().is_empty() {
            let text = sanitize_subtitle_text(&raw_text);
            if !text.is_empty() {
                chunks.push(SubtitleChunk { raw_text, text });
            }
        }

        start = end;
        while start < chars.len() && chars[start].is_whitespace() {
            start += 1;
        }
    }

    if chunks.is_empty() {
        vec![SubtitleChunk {
            raw_text: normalized,
            text: sanitized,
        }]
    } else {
        chunks
    }
}

fn ensure_max_duration_chunks(
    mut chunks: Vec<SubtitleChunk>,
    total_duration_ms: i64,
    max_chars: usize,
) -> Vec<SubtitleChunk> {
    if chunks.is_empty() || total_duration_ms <= 0 || max_chars == 0 {
        return chunks;
    }

    loop {
        let durations = calculate_proportional_durations(&chunks, total_duration_ms);
        let mut changed = false;

        for (index, duration) in durations.iter().enumerate() {
            if *duration <= MAX_SUBTITLE_DURATION_MS {
                continue;
            }

            let char_count = chunks[index].text.chars().count();
            if char_count <= 1 {
                continue;
            }

            let required_pieces = div_ceil_i64(*duration, MAX_SUBTITLE_DURATION_MS).max(2) as usize;
            let per_piece_limit = max_chars
                .min(div_ceil_usize(char_count, required_pieces))
                .max(1);
            let split_chunks = split_subtitle_text(&chunks[index].raw_text, per_piece_limit);

            if split_chunks.len() > 1 {
                chunks.splice(index..=index, split_chunks);
                changed = true;
                break;
            }
        }

        if !changed {
            break;
        }
    }

    chunks
}

fn merge_chunks_to_fit_duration(
    mut chunks: Vec<SubtitleChunk>,
    total_duration_ms: i64,
    max_chars: usize,
) -> Vec<SubtitleChunk> {
    let max_chunk_count = total_duration_ms.max(1) as usize;
    while chunks.len() > max_chunk_count && chunks.len() > 1 {
        chunks = merge_chunks_once_best(chunks, max_chars);
    }
    chunks
}

fn merge_chunks_once_best(mut chunks: Vec<SubtitleChunk>, max_chars: usize) -> Vec<SubtitleChunk> {
    if chunks.len() < 2 {
        return chunks;
    }

    let merge_index = find_best_merge_index(&chunks, max_chars).unwrap_or(chunks.len() - 2);
    let merged = merge_adjacent_chunks(&chunks[merge_index], &chunks[merge_index + 1]);
    chunks.splice(merge_index..=merge_index + 1, std::iter::once(merged));
    chunks
}

fn find_best_merge_index(chunks: &[SubtitleChunk], max_chars: usize) -> Option<usize> {
    if chunks.len() < 2 {
        return None;
    }

    let mut best_index = None;
    let mut best_score = None;

    for index in 0..chunks.len() - 1 {
        let merged = merge_adjacent_chunks(&chunks[index], &chunks[index + 1]);
        let merged_len = merged.text.chars().count();
        let overflow = merged_len.saturating_sub(max_chars);
        let balance = merged_len.abs_diff(max_chars);
        let score = (overflow, balance, merged_len, index);

        if best_score.is_none_or(|current| score < current) {
            best_score = Some(score);
            best_index = Some(index);
        }
    }

    best_index
}

fn merge_adjacent_chunks(left: &SubtitleChunk, right: &SubtitleChunk) -> SubtitleChunk {
    let raw_text = format!("{}{}", left.raw_text, right.raw_text);
    let text = sanitize_subtitle_text(&raw_text);
    SubtitleChunk { raw_text, text }
}

fn heal_split_artifacts(
    mut entries: Vec<SubtitleEntry>,
    max_chars: usize,
) -> Result<Vec<SubtitleEntry>, String> {
    if entries.len() < 2 || max_chars == 0 {
        return Ok(entries);
    }

    let mut index = 0;
    while index < entries.len() {
        if merge_short_artifact(&mut entries, index, max_chars)? {
            index = index.saturating_sub(1);
            continue;
        }

        if move_leading_continuation_to_previous(&mut entries, index, max_chars)? {
            index = index.saturating_sub(1);
            continue;
        }

        index += 1;
    }

    Ok(entries)
}

fn merge_short_artifact(
    entries: &mut Vec<SubtitleEntry>,
    index: usize,
    max_chars: usize,
) -> Result<bool, String> {
    let current_len = entries[index].text.chars().count();
    if current_len == 0 || current_len > 2 {
        return Ok(false);
    }

    if should_merge_short_with_next(&entries[index].text) {
        if merge_with_next_if_possible(entries, index, max_chars)? {
            return Ok(true);
        }
    }

    if merge_with_previous_if_possible(entries, index, max_chars)? {
        return Ok(true);
    }

    merge_with_next_if_possible(entries, index, max_chars)
}

fn should_merge_short_with_next(text: &str) -> bool {
    matches!(text, "あ" | "で" | "じゃ" | "じゃあ")
}

fn merge_with_previous_if_possible(
    entries: &mut Vec<SubtitleEntry>,
    index: usize,
    max_chars: usize,
) -> Result<bool, String> {
    if index == 0 || !entries_are_close(&entries[index - 1], &entries[index])? {
        return Ok(false);
    }

    let merged_text = format!("{}{}", entries[index - 1].text, entries[index].text);
    if merged_text.chars().count() > max_chars {
        return Ok(false);
    }

    entries[index - 1].text = merged_text;
    entries[index - 1].end_time = entries[index].end_time.clone();
    entries.remove(index);
    Ok(true)
}

fn merge_with_next_if_possible(
    entries: &mut Vec<SubtitleEntry>,
    index: usize,
    max_chars: usize,
) -> Result<bool, String> {
    if index + 1 >= entries.len() || !entries_are_close(&entries[index], &entries[index + 1])? {
        return Ok(false);
    }

    let merged_text = format!("{}{}", entries[index].text, entries[index + 1].text);
    if merged_text.chars().count() > max_chars {
        return Ok(false);
    }

    entries[index + 1].text = merged_text;
    entries[index + 1].start_time = entries[index].start_time.clone();
    entries.remove(index);
    Ok(true)
}

fn move_leading_continuation_to_previous(
    entries: &mut Vec<SubtitleEntry>,
    index: usize,
    max_chars: usize,
) -> Result<bool, String> {
    if index == 0 || !entries_are_close(&entries[index - 1], &entries[index])? {
        return Ok(false);
    }

    let Some(prefix) = leading_continuation_prefix(&entries[index].text) else {
        return Ok(false);
    };

    let previous_len = entries[index - 1].text.chars().count();
    let prefix_len = prefix.chars().count();
    let current_len = entries[index].text.chars().count();
    if previous_len + prefix_len > max_chars || current_len <= prefix_len {
        return Ok(false);
    }

    let current_start = parse_time_to_millis(&entries[index].start_time)?;
    let current_end = parse_time_to_millis(&entries[index].end_time)?;
    let current_duration = current_end - current_start;
    if current_duration <= 1 {
        return Ok(false);
    }

    let moved_duration =
        (current_duration * prefix_len as i64 / current_len as i64).clamp(1, current_duration - 1);
    let new_boundary = format_millis_to_time(current_start + moved_duration);
    let remaining = entries[index].text[prefix.len()..].to_string();

    entries[index - 1].text.push_str(prefix);
    entries[index - 1].end_time = new_boundary.clone();
    entries[index].text = remaining;
    entries[index].start_time = new_boundary;
    Ok(true)
}

fn leading_continuation_prefix(text: &str) -> Option<&'static str> {
    const PREFIXES: &[&str] = &["れて", "れた", "る", "た"];

    PREFIXES.iter().copied().find(|prefix| text.starts_with(prefix))
}

fn entries_are_close(left: &SubtitleEntry, right: &SubtitleEntry) -> Result<bool, String> {
    let left_end = parse_time_to_millis(&left.end_time)?;
    let right_start = parse_time_to_millis(&right.start_time)?;

    Ok((0..=MAX_HEAL_GAP_MS).contains(&(right_start - left_end)))
}

fn calculate_proportional_durations(chunks: &[SubtitleChunk], total_duration_ms: i64) -> Vec<i64> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let weights: Vec<i64> = chunks
        .iter()
        .map(|chunk| chunk.text.chars().count().max(1) as i64)
        .collect();
    let total_weight = weights.iter().sum::<i64>().max(1);

    let mut durations = vec![0; chunks.len()];
    let mut assigned = 0;
    let mut remainders = Vec::with_capacity(chunks.len());

    for (index, weight) in weights.iter().enumerate() {
        let numerator = total_duration_ms * *weight;
        let base = numerator / total_weight;
        durations[index] = base;
        assigned += base;
        remainders.push((index, numerator % total_weight));
    }

    remainders.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut remaining = total_duration_ms - assigned;
    for (index, _) in remainders {
        if remaining <= 0 {
            break;
        }
        durations[index] += 1;
        remaining -= 1;
    }

    for index in 0..durations.len() {
        if durations[index] > 0 {
            continue;
        }

        if let Some(donor_index) = durations
            .iter()
            .enumerate()
            .filter(|(_, duration)| **duration > 1)
            .max_by_key(|(_, duration)| **duration)
            .map(|(donor_index, _)| donor_index)
        {
            durations[donor_index] -= 1;
            durations[index] += 1;
        }
    }

    durations
}

fn rebalance_short_durations(durations: &mut [i64]) {
    if durations.len() < 2 {
        return;
    }

    for index in 0..durations.len() {
        if durations[index] >= MIN_SUBTITLE_DURATION_MS {
            continue;
        }

        let mut shortage = MIN_SUBTITLE_DURATION_MS - durations[index];

        while shortage > 0 {
            let mut candidates = Vec::new();

            if index > 0 && durations[index - 1] > MIN_SUBTITLE_DURATION_MS {
                candidates.push(index - 1);
            }

            if index + 1 < durations.len() && durations[index + 1] > MIN_SUBTITLE_DURATION_MS {
                candidates.push(index + 1);
            }

            let Some(donor_index) = candidates
                .into_iter()
                .max_by_key(|candidate| durations[*candidate])
            else {
                break;
            };

            let transfer = shortage.min(durations[donor_index] - MIN_SUBTITLE_DURATION_MS);
            durations[donor_index] -= transfer;
            durations[index] += transfer;
            shortage -= transfer;
        }
    }
}

fn build_split_entries(entry: SubtitleEntry, max_chars: usize) -> Result<Vec<SubtitleEntry>, String> {
    let start_ms = parse_time_to_millis(&entry.start_time)?;
    let end_ms = parse_time_to_millis(&entry.end_time)?;
    let total_duration_ms = end_ms - start_ms;

    if total_duration_ms <= 0 {
        return Ok(Vec::new());
    }

    let mut chunks = split_subtitle_text(&entry.text, max_chars);
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    chunks = ensure_max_duration_chunks(chunks, total_duration_ms, max_chars);
    chunks = merge_chunks_to_fit_duration(chunks, total_duration_ms, max_chars);

    let mut durations = calculate_proportional_durations(&chunks, total_duration_ms);
    while durations.iter().any(|duration| *duration <= 0) && chunks.len() > 1 {
        chunks = merge_chunks_once_best(chunks, max_chars);
        durations = calculate_proportional_durations(&chunks, total_duration_ms);
    }

    rebalance_short_durations(&mut durations);

    let mut split_entries = Vec::with_capacity(chunks.len());
    let mut current_start = start_ms;

    for (index, chunk) in chunks.iter().enumerate() {
        let current_end = if index == chunks.len() - 1 {
            end_ms
        } else {
            current_start + durations[index]
        };

        split_entries.push(SubtitleEntry {
            index: 0,
            start_time: format_millis_to_time(current_start),
            end_time: format_millis_to_time(current_end),
            text: chunk.text.clone(),
        });

        current_start = current_end;
    }

    Ok(split_entries)
}

fn parse_time_to_millis(time_str: &str) -> Result<i64, String> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("無効な時間形式です: {}", time_str));
    }

    let hours: i64 = parts[0].parse().map_err(|_| "時間のパースに失敗しました")?;
    let minutes: i64 = parts[1].parse().map_err(|_| "分のパースに失敗しました")?;

    let sec_parts: Vec<&str> = parts[2].split(',').collect();
    if sec_parts.len() != 2 {
        return Err(format!(
            "無効な秒形式です（カンマが必要です）: {}",
            parts[2]
        ));
    }

    let seconds: i64 = sec_parts[0].parse().map_err(|_| "秒のパースに失敗しました")?;
    let millis: i64 = sec_parts[1].parse().map_err(|_| "ミリ秒のパースに失敗しました")?;

    Ok(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis)
}

fn format_millis_to_time(total_millis: i64) -> String {
    let total_millis = total_millis.max(0);
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis % 3_600_000) / 60_000;
    let seconds = (total_millis % 60_000) / 1_000;
    let millis = total_millis % 1_000;

    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, seconds, millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn entry(index: usize, start_time: &str, end_time: &str, text: &str) -> SubtitleEntry {
        SubtitleEntry {
            index,
            start_time: start_time.to_string(),
            end_time: end_time.to_string(),
            text: text.to_string(),
        }
    }

    fn temp_srt_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "movie2text-test-{}-{}.srt",
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn short_subtitle_is_not_split() {
        let splits = build_split_entries(
            entry(1, "00:00:00,000", "00:00:03,000", "これは短い字幕です"),
            23,
        )
        .unwrap();

        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0].text, "これは短い字幕です");
    }

    #[test]
    fn long_subtitle_is_split_into_multiple_entries() {
        let splits = build_split_entries(
            entry(
                1,
                "00:00:00,000",
                "00:00:06,000",
                "今日は雨です。明日は晴れです。週末はまた天気が変わります。",
            ),
            10,
        )
        .unwrap();

        assert!(splits.len() > 1);
    }

    #[test]
    fn each_split_stays_within_max_chars_when_timing_allows_it() {
        let splits = build_split_entries(
            entry(
                1,
                "00:00:00,000",
                "00:00:08,000",
                "字幕をPremiere向けに短いエントリへ自動で整形します",
            ),
            8,
        )
        .unwrap();

        assert!(splits.iter().all(|split| split.text.chars().count() <= 8));
    }

    #[test]
    fn punctuation_is_removed_from_final_text() {
        let splits = build_split_entries(
            entry(
                1,
                "00:00:00,000",
                "00:00:04,000",
                "はい、そうです。大丈夫です！",
            ),
            8,
        )
        .unwrap();

        assert!(splits.iter().all(|split| {
            !split
                .text
                .chars()
                .any(|c| matches!(c, '。' | '、' | '！' | '？' | '!' | '?' | '…' | '・'))
        }));
    }

    #[test]
    fn punctuation_is_preferred_as_split_candidate() {
        let splits = build_split_entries(
            entry(
                1,
                "00:00:00,000",
                "00:00:04,000",
                "今日は雨です。明日は晴れです",
            ),
            8,
        )
        .unwrap();

        let texts: Vec<&str> = splits.iter().map(|split| split.text.as_str()).collect();
        assert_eq!(texts, vec!["今日は雨です", "明日は晴れです"]);
    }

    #[test]
    fn text_is_force_split_when_no_natural_candidate_exists() {
        let chunks = split_subtitle_text("abcdefghij", 4);
        let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.text.as_str()).collect();
        assert_eq!(texts, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn entries_are_renumbered_after_split() {
        let entries = vec![
            entry(10, "00:00:00,000", "00:00:04,000", "これはかなり長い字幕なので分割されます"),
            entry(25, "00:00:04,000", "00:00:06,000", "短い字幕"),
        ];

        let rewritten = apply_character_limit_to_entries(entries, 8).unwrap();
        let indexes: Vec<usize> = rewritten.iter().map(|entry| entry.index).collect();

        assert_eq!(indexes, (1..=indexes.len()).collect::<Vec<_>>());
    }

    #[test]
    fn split_timings_stay_in_original_range_without_overlap() {
        let splits = build_split_entries(
            entry(
                1,
                "00:00:10,000",
                "00:00:16,000",
                "今日は雨です。明日は晴れです。週末は曇りです。",
            ),
            8,
        )
        .unwrap();

        let original_start = parse_time_to_millis("00:00:10,000").unwrap();
        let original_end = parse_time_to_millis("00:00:16,000").unwrap();

        for (index, split) in splits.iter().enumerate() {
            let start = parse_time_to_millis(&split.start_time).unwrap();
            let end = parse_time_to_millis(&split.end_time).unwrap();

            assert!(start >= original_start);
            assert!(end <= original_end);
            assert!(start < end);

            if index > 0 {
                let previous_end = parse_time_to_millis(&splits[index - 1].end_time).unwrap();
                assert!(previous_end <= start);
            }
        }

        assert_eq!(splits.last().unwrap().end_time, "00:00:16,000");
    }

    #[test]
    fn empty_or_punctuation_only_subtitles_do_not_crash() {
        let entries = vec![
            entry(1, "00:00:00,000", "00:00:02,000", "\n\n"),
            entry(2, "00:00:02,000", "00:00:04,000", "。。。！！"),
            entry(3, "00:00:04,000", "00:00:06,000", "有効な字幕です"),
        ];

        let rewritten = apply_character_limit_to_entries(entries, 5).unwrap();

        assert_eq!(rewritten.len(), 2);
        assert!(rewritten.iter().all(|entry| !entry.text.is_empty()));
    }

    #[test]
    fn trailing_short_particles_are_merged_with_previous_entry() {
        let entries = vec![
            entry(1, "00:00:29,420", "00:00:31,030", "大阪行く予定のある人ぶく"),
            entry(2, "00:00:31,030", "00:00:32,040", "ましといた方がいいです"),
            entry(3, "00:00:32,040", "00:00:32,640", "よ"),
        ];

        let rewritten = apply_character_limit_to_entries(entries, 23).unwrap();
        let texts: Vec<&str> = rewritten.iter().map(|entry| entry.text.as_str()).collect();

        assert_eq!(texts, vec!["大阪行く予定のある人ぶく", "ましといた方がいいですよ"]);
        assert!(rewritten.iter().all(|entry| entry.text.chars().count() <= 23));
    }

    #[test]
    fn leading_continuation_is_moved_to_previous_entry() {
        let entries = vec![
            entry(1, "00:00:40,279", "00:00:42,869", "ごぼうのやつ乗って"),
            entry(2, "00:00:42,869", "00:00:44,020", "るラーメンのお店"),
        ];

        let rewritten = apply_character_limit_to_entries(entries, 23).unwrap();
        let texts: Vec<&str> = rewritten.iter().map(|entry| entry.text.as_str()).collect();

        assert_eq!(texts, vec!["ごぼうのやつ乗ってる", "ラーメンのお店"]);
        assert!(rewritten[0].end_time > "00:00:42,869".to_string());
        assert_eq!(rewritten[0].end_time, rewritten[1].start_time);
    }

    #[test]
    fn leading_discourse_markers_are_merged_with_next_entry() {
        let entries = vec![
            entry(1, "00:10:54,700", "00:10:56,680", "この速度をゆっくりにして"),
            entry(2, "00:10:56,680", "00:10:57,280", "で"),
            entry(3, "00:10:57,280", "00:10:59,182", "どこが繋がってるか研究して"),
        ];

        let rewritten = apply_character_limit_to_entries(entries, 23).unwrap();
        let texts: Vec<&str> = rewritten.iter().map(|entry| entry.text.as_str()).collect();

        assert_eq!(texts, vec!["この速度をゆっくりにして", "でどこが繋がってるか研究して"]);
        assert!(rewritten.iter().all(|entry| entry.text.chars().count() <= 23));
    }

    #[test]
    fn max_duration_triggers_additional_splitting() {
        let splits =
            build_split_entries(entry(1, "00:00:00,000", "00:00:10,000", "abcdefghij"), 23)
                .unwrap();

        let durations: Vec<i64> = splits
            .iter()
            .map(|split| {
                parse_time_to_millis(&split.end_time).unwrap()
                    - parse_time_to_millis(&split.start_time).unwrap()
            })
            .collect();

        assert!(splits.len() >= 4);
        assert!(durations
            .iter()
            .all(|duration| *duration <= MAX_SUBTITLE_DURATION_MS));
    }

    #[test]
    fn very_short_duration_merges_chunks_instead_of_dropping_text() {
        let splits =
            build_split_entries(entry(1, "00:00:00,000", "00:00:00,002", "abcdefghij"), 1)
                .unwrap();

        let combined = splits.iter().map(|split| split.text.as_str()).collect::<String>();

        assert_eq!(combined, "abcdefghij");
        assert_eq!(splits.len(), 2);
        assert!(splits.iter().all(|split| split.start_time < split.end_time));
    }

    #[test]
    fn parse_and_render_keep_srt_format_compatible() {
        let path = temp_srt_path();
        let original_entries = vec![
            entry(1, "00:00:00,000", "00:00:01,500", "1行目"),
            entry(2, "00:00:01,500", "00:00:03,000", "2行目\n改行あり"),
        ];

        fs::write(&path, render_srt(&original_entries)).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let parsed = parse_srt(&content).unwrap();

        fs::remove_file(path).unwrap();

        assert_eq!(parsed, original_entries);
    }
}
